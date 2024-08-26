use std::{
    collections::{BTreeMap, HashMap},
    io::{self, SeekFrom},
    path::PathBuf,
};

use async_trait::async_trait;
use gpt::{mbr::ProtectiveMBR, partition::Partition, DiskDevice, GptConfig, GptDisk};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{
    fs::File,
    io::{AsyncSeekExt, AsyncWriteExt},
    task::block_in_place,
};
use uuid::Uuid;

use crate::managers::application::{ApplicationDisk, ApplicationError};
#[derive(Error, Debug, PartialEq, PartialOrd, Clone, Serialize, Deserialize)]
pub enum ApplicationDiskManagerError {
    #[error("Requested empty partition.")]
    RequestedEmptyPartition(),
    #[error("Requested too big partition: {0}")]
    RequestedTooBigPartition(String),
    #[error("Can't create disk: {0}")]
    DiskCreation(String),
    #[error("Can't delete disk: {0}")]
    DiskDeletion(String),
    #[error("Failed to write MBA: {0}")]
    PartitionMetadataWrite(String),
    #[error("Failed to configure GPT: {0}")]
    GPTConfiguration(String),
    #[error("Failed to update GPT: {0}")]
    GPTUpdate(String),
    #[error("Failed to create partition: {0}")]
    PartitionCreation(String),
    #[error("Failed to read GPT: {0}")]
    GPTRead(String),
    #[error("Failed to write GPT configuration: {0}")]
    GPTSave(String),
    #[error("Failed to save file: {0}")]
    FileShutdown(String),
    #[error("Failed to read partition size: {0}")]
    GetPartitionSize(String),
    #[error("Missing Data partition.")]
    DataPartitionNotFound(),
    #[error("Missing Image partition.")]
    ImagePartitionNotFound(),
    #[error("Data partition size is incorrect.")]
    DataPartitionSizeIncorrect(),
    #[error("Image partition size is incorrect.")]
    ImagePartitionSizeIncorrect(),
}

pub struct ApplicationDiskManager {
    file_path: PathBuf,
    image_part_bytes_size: u64,
    data_part_bytes_size: u64,
}

#[async_trait]
impl ApplicationDisk for ApplicationDiskManager {
    async fn create_disk_with_partitions(&self) -> Result<(), ApplicationError> {
        if self.ensure_partitions_correctness().await.is_err() {
            self.create_application_disk()
                .await
                .map_err(|err| ApplicationError::DiskOpertaion(err.to_string()))?
        }
        Ok(())
    }
    async fn update_disk_with_partitions(
        &mut self,
        new_data_part_size_mb: u32,
        new_image_part_size_mb: u32,
    ) -> Result<(), ApplicationError> {
        self.update_partitions_if_sizes_differ(new_data_part_size_mb, new_image_part_size_mb)
            .await
            .map_err(|err| ApplicationError::DiskOpertaion(err.to_string()))
    }
    async fn get_data_partition_uuid(&self) -> Result<Uuid, ApplicationError> {
        let partitions = self
            .read_partitions()
            .map_err(|err| ApplicationError::DiskOpertaion(err.to_string()))?;
        Ok(self
            .get_data_partition(&partitions)
            .map_err(|err| ApplicationError::DiskOpertaion(err.to_string()))?
            .part_guid)
    }
    async fn get_image_partition_uuid(&self) -> Result<Uuid, ApplicationError> {
        let partitions = self
            .read_partitions()
            .map_err(|err| ApplicationError::DiskOpertaion(err.to_string()))?;
        Ok(self
            .get_image_partition(&partitions)
            .map_err(|err| ApplicationError::DiskOpertaion(err.to_string()))?
            .part_guid)
    }
}

impl ApplicationDiskManager {
    pub const DISK_NAME: &'static str = "disk.raw";
    const IMAGE_PARTITION: &'static str = "image";
    const DATA_PARTITION: &'static str = "data";
    const LBA_SIZE: gpt::disk::LogicalBlockSize = gpt::disk::LogicalBlockSize::Lb512;
    const MAX_PARTITION_SIZE_MB: u32 = 100 * 1024; // 100 GB

    pub fn new(
        mut workdir_path: PathBuf,
        image_partition_size_mb: u32,
        data_partition_size_mb: u32,
    ) -> Result<Self, ApplicationDiskManagerError> {
        let data_partition_size = Self::validate_and_convert_to_bytes(data_partition_size_mb)?;
        let image_partition_size = Self::validate_and_convert_to_bytes(image_partition_size_mb)?;
        workdir_path.push(Self::DISK_NAME);
        Ok(Self {
            file_path: workdir_path,
            image_part_bytes_size: image_partition_size,
            data_part_bytes_size: data_partition_size,
        })
    }

    async fn ensure_partitions_correctness(&self) -> Result<(), ApplicationDiskManagerError> {
        let partitions = self.read_partitions()?;
        let image_partition = self.get_image_partition(&partitions)?;
        let data_partition = self.get_data_partition(&partitions)?;
        if image_partition
            .bytes_len(Self::LBA_SIZE)
            .map_err(|err| ApplicationDiskManagerError::GetPartitionSize(err.to_string()))?
            != self.image_part_bytes_size
        {
            Err(ApplicationDiskManagerError::ImagePartitionSizeIncorrect())
        } else if data_partition
            .bytes_len(Self::LBA_SIZE)
            .map_err(|err| ApplicationDiskManagerError::GetPartitionSize(err.to_string()))?
            != self.data_part_bytes_size
        {
            Err(ApplicationDiskManagerError::DataPartitionSizeIncorrect())
        } else {
            Ok(())
        }
    }

    async fn update_partitions_if_sizes_differ(
        &mut self,
        new_data_part_size_mb: u32,
        new_image_part_size_mb: u32,
    ) -> Result<(), ApplicationDiskManagerError> {
        self.ensure_partitions_correctness().await?;
        let new_image_part_bytes_size =
            Self::validate_and_convert_to_bytes(new_image_part_size_mb)?;
        let new_data_part_bytes_size = Self::validate_and_convert_to_bytes(new_data_part_size_mb)?;
        if new_image_part_bytes_size != self.image_part_bytes_size
            || new_data_part_bytes_size != self.data_part_bytes_size
        {
            self.image_part_bytes_size = new_image_part_bytes_size;
            self.data_part_bytes_size = new_data_part_bytes_size;
            self.create_application_disk().await?;
        }
        Ok(())
    }

    async fn create_application_disk(&self) -> Result<(), ApplicationDiskManagerError> {
        let mut file = self
            .create_disk_device()
            .await
            .map_err(|err| ApplicationDiskManagerError::DiskCreation(err.to_string()))?;
        block_in_place(|| self.create_gpt_and_partitions(&mut file))?;

        Self::sync_file(file).await?;
        self.ensure_partitions_correctness().await
    }

    fn validate_and_convert_to_bytes(
        partition_size_mb: u32,
    ) -> Result<u64, ApplicationDiskManagerError> {
        if partition_size_mb > Self::MAX_PARTITION_SIZE_MB {
            Err(ApplicationDiskManagerError::RequestedTooBigPartition(
                format!(
                    "Requested size is: {}, maximum size is: {}.",
                    partition_size_mb,
                    Self::MAX_PARTITION_SIZE_MB
                ),
            ))
        } else if partition_size_mb == 0 {
            Err(ApplicationDiskManagerError::RequestedEmptyPartition())
        } else {
            Ok(partition_size_mb as u64 * 1024 * 1024)
        }
    }

    fn get_image_partition(
        &self,
        partitions: &HashMap<String, Partition>,
    ) -> Result<Partition, ApplicationDiskManagerError> {
        Ok(partitions
            .get(Self::IMAGE_PARTITION)
            .ok_or(ApplicationDiskManagerError::ImagePartitionNotFound())?
            .clone())
    }

    fn get_data_partition(
        &self,
        partitions: &HashMap<String, Partition>,
    ) -> Result<Partition, ApplicationDiskManagerError> {
        Ok(partitions
            .get(Self::DATA_PARTITION)
            .ok_or(ApplicationDiskManagerError::DataPartitionNotFound())?
            .clone())
    }

    fn read_partitions(&self) -> Result<HashMap<String, Partition>, ApplicationDiskManagerError> {
        let gpt = block_in_place(|| {
            GptConfig::default()
                .writable(false)
                .initialized(true)
                .open(&self.file_path)
        })
        .map_err(|err| ApplicationDiskManagerError::GPTRead(err.to_string()))?;

        Ok(gpt
            .partitions()
            .iter()
            .map(|(_id, value)| (value.name.clone(), value.clone()))
            .collect())
    }

    async fn sync_file(file: std::fs::File) -> Result<(), ApplicationDiskManagerError> {
        let mut file = File::from_std(file);
        file.sync_all()
            .await
            .map_err(|err| ApplicationDiskManagerError::PartitionCreation(err.to_string()))?;
        file.shutdown()
            .await
            .map_err(|err| ApplicationDiskManagerError::FileShutdown(err.to_string()))
    }

    async fn create_disk_device(&self) -> Result<std::fs::File, io::Error> {
        let mut file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.file_path)
            .await?;
        file.seek(SeekFrom::Start(self.calculate_total_size() - 1))
            .await?;
        file.write_all(&[0u8]).await?;
        file.seek(SeekFrom::Start(0u64)).await?;
        Ok(file.into_std().await)
    }

    fn calculate_total_size(&self) -> u64 {
        const PRIMARY_GPT_LBA_SECTORS: u64 = 34;
        const BACKUP_GPT_LBA_SECTORS: u64 = 34;
        self.data_part_bytes_size
            + self.image_part_bytes_size
            + ((PRIMARY_GPT_LBA_SECTORS + BACKUP_GPT_LBA_SECTORS) * u64::from(Self::LBA_SIZE))
    }

    fn write_mbr(
        &self,
        file: &mut Box<&mut dyn DiskDevice>,
    ) -> Result<(), ApplicationDiskManagerError> {
        let mbr = ProtectiveMBR::with_lb_size(
            (self.calculate_total_size() / u64::from(Self::LBA_SIZE) - 1) as u32,
        );
        mbr.overwrite_lba0(file)
            .map_err(|err| ApplicationDiskManagerError::PartitionMetadataWrite(err.to_string()))?;
        Ok(())
    }

    fn create_partition(
        gpt: &mut GptDisk,
        name: impl AsRef<str>,
        partition_size_bytes: u64,
    ) -> Result<(), ApplicationDiskManagerError> {
        gpt.add_partition(
            name.as_ref(),
            partition_size_bytes,
            gpt::partition_types::LINUX_FS,
            0,
            None,
        )
        .map(|_| ())
        .map_err(|err| ApplicationDiskManagerError::PartitionCreation(err.to_string()))
    }

    fn create_gpt_and_partitions(
        &self,
        file: &mut impl DiskDevice,
    ) -> Result<(), ApplicationDiskManagerError> {
        let mut file: Box<&mut dyn DiskDevice> = Box::new(file);
        self.write_mbr(&mut file)?;

        let mut gpt = GptConfig::default()
            .initialized(false)
            .writable(true)
            .logical_block_size(Self::LBA_SIZE)
            .create_from_device(file, None)
            .map_err(|err| ApplicationDiskManagerError::GPTConfiguration(err.to_string()))?;
        gpt.update_partitions(BTreeMap::new())
            .map_err(|err| ApplicationDiskManagerError::GPTUpdate(err.to_string()))?;

        Self::create_partition(&mut gpt, Self::IMAGE_PARTITION, self.image_part_bytes_size)?;
        Self::create_partition(&mut gpt, Self::DATA_PARTITION, self.data_part_bytes_size)?;

        gpt.write()
            .map_err(|err| ApplicationDiskManagerError::GPTSave(err.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::{
        fs::{create_dir, remove_dir_all},
        path::PathBuf,
        str::FromStr,
    };

    use crate::{
        managers::application::ApplicationDisk,
        storage::app_disk_manager::ApplicationDiskManagerError,
    };

    use super::ApplicationDiskManager;

    struct FilePathHolder {
        disk_file_path: &'static str,
    }

    impl FilePathHolder {
        fn new() -> Self {
            const STR_PATH: &str =
                "/tmp/partition-test-disk-file-path-0d25e483-ad0f-4089-86df-23b5d60117f8";
            create_dir(STR_PATH).unwrap();

            FilePathHolder {
                disk_file_path: &STR_PATH,
            }
        }
    }

    impl Drop for FilePathHolder {
        fn drop(&mut self) {
            let _ = remove_dir_all(self.disk_file_path);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn create_disk_and_partition() {
        let path_holder = FilePathHolder::new();
        let path = PathBuf::from_str(path_holder.disk_file_path).unwrap();
        let mut disk_manager = ApplicationDiskManager::new(path, 10, 10).unwrap();
        assert!(disk_manager.create_disk_with_partitions().await.is_ok());
        assert!(disk_manager.get_data_partition_uuid().await.is_ok());
        assert!(disk_manager.get_image_partition_uuid().await.is_ok());
        assert!(disk_manager
            .update_disk_with_partitions(20, 40)
            .await
            .is_ok());
        assert!(disk_manager.get_data_partition_uuid().await.is_ok());
        assert!(disk_manager.get_image_partition_uuid().await.is_ok());
        let partitions = disk_manager.read_partitions().unwrap();
        assert_eq!(
            disk_manager
                .get_data_partition(&partitions)
                .unwrap()
                .bytes_len(ApplicationDiskManager::LBA_SIZE)
                .unwrap(),
            20 * 1024 * 1024
        );
        assert_eq!(
            disk_manager
                .get_image_partition(&partitions)
                .unwrap()
                .bytes_len(ApplicationDiskManager::LBA_SIZE)
                .unwrap(),
            40 * 1024 * 1024
        );
    }

    #[test]
    fn calculate_total_size() {
        const IMAGE_PART_SIZE_MB: u32 = 1;
        const DATA_PART_SIZE_MB: u32 = 1;
        let disk_manager = ApplicationDiskManager::new(
            PathBuf::from_str(".").unwrap(),
            IMAGE_PART_SIZE_MB,
            DATA_PART_SIZE_MB,
        )
        .unwrap();
        assert_eq!(
            disk_manager.calculate_total_size(),
            IMAGE_PART_SIZE_MB as u64 * 1024 * 1024
                + DATA_PART_SIZE_MB as u64 * 1024 * 1024
                + 34 as u64 * 2 * u64::from(ApplicationDiskManager::LBA_SIZE)
        );
    }

    #[test]
    fn validate_and_convert_to_bytes() {
        const PARTITION_SIZE_MB: u32 = 1;
        assert_eq!(
            ApplicationDiskManager::validate_and_convert_to_bytes(PARTITION_SIZE_MB).unwrap(),
            PARTITION_SIZE_MB as u64 * 1024 * 1024
        );
    }

    #[test]
    fn validate_and_convert_to_bytes_to_much_space_requested() {
        const PARTITION_SIZE_MB: u32 = 1024 * 100 + 1; // Exceeding limit by 1 MB
        assert!(matches!(
            ApplicationDiskManager::validate_and_convert_to_bytes(PARTITION_SIZE_MB),
            Err(ApplicationDiskManagerError::RequestedTooBigPartition(_))
        ));
    }

    #[test]
    fn validate_and_convert_to_bytes_empty_space() {
        const PARTITION_SIZE_MB: u32 = 0;
        assert!(matches!(
            ApplicationDiskManager::validate_and_convert_to_bytes(PARTITION_SIZE_MB),
            Err(ApplicationDiskManagerError::RequestedEmptyPartition())
        ));
    }
}
