use std::{
    collections::{BTreeMap, HashMap},
    io::SeekFrom,
    path::PathBuf,
};

use gpt::{mbr::ProtectiveMBR, DiskDevice, GptConfig, GptDisk};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{
    fs::File,
    io::{AsyncSeekExt, AsyncWriteExt},
};
use uuid::Uuid;

use crate::managers::application::ApplicationDiskData;

#[derive(Error, Debug, PartialEq, PartialOrd, Clone, Serialize, Deserialize)]
pub enum ApplicationDiskManagerError {
    #[error("Requested empty partition.")]
    RequestedEmptyPartition(),
    #[error("Requested too big partition: {0}")]
    RequestedTooBigPartition(String),
    #[error("Can't create disk image: {0}")]
    DiskCreation(String),
    #[error("Failed to write MBA: {0}")]
    PartitionMetadataWrite(String),
    #[error("Failed to configure GPT: {0}")]
    GPTConfiguration(String),
    #[error("Failed to create partition: {0}")]
    PartitionCreation(String),
    #[error("Missing partition: {0}")]
    MissingPartition(String),
}

pub struct ApplicationDiskManager {
    file_path: PathBuf,
    image_partition_size: u64,
    data_partition_size: u64,
}

impl ApplicationDiskManager {
    const DISK_NAME: &'static str = "disk.raw";
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
            image_partition_size,
            data_partition_size,
        })
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
            Ok((partition_size_mb * 1024 * 1024).into())
        }
    }

    pub async fn create_application_disk(
        &self,
    ) -> Result<ApplicationDiskData, ApplicationDiskManagerError> {
        let file = self.create_disk_device().await?;
        self.create_gpt_and_partitions(&file)?;
        Self::sync_file(file).await?;

        self.load_paritions_uuids_from_disk()
    }

    fn load_paritions_uuids_from_disk(
        &self,
    ) -> Result<ApplicationDiskData, ApplicationDiskManagerError> {
        let partitions_data = self.read_partitions_uuids()?;
        Ok(ApplicationDiskData {
            image_partition_uuid: *partitions_data.get(Self::IMAGE_PARTITION).ok_or(
                ApplicationDiskManagerError::MissingPartition("no Image partition".to_string()),
            )?,
            data_partition_uuid: *partitions_data.get(Self::DATA_PARTITION).ok_or(
                ApplicationDiskManagerError::MissingPartition("no Data partition".to_string()),
            )?,
        })
    }

    fn read_partitions_uuids(&self) -> Result<HashMap<String, Uuid>, ApplicationDiskManagerError> {
        let gpt = GptConfig::default()
            .writable(false)
            .initialized(true)
            .open(&self.file_path)
            .map_err(|err| ApplicationDiskManagerError::GPTConfiguration(err.to_string()))?;

        Ok(gpt
            .partitions()
            .iter()
            .map(|(_id, value)| (value.name.clone(), value.part_guid))
            .collect())
    }

    async fn sync_file(file: std::fs::File) -> Result<(), ApplicationDiskManagerError> {
        let file = File::from_std(file);
        file.sync_all()
            .await
            .map_err(|err| ApplicationDiskManagerError::PartitionCreation(err.to_string()))
    }

    async fn create_disk_device(&self) -> Result<std::fs::File, ApplicationDiskManagerError> {
        let mut file = File::create_new(&self.file_path)
            .await
            .map_err(|err| ApplicationDiskManagerError::DiskCreation(err.to_string()))?;
        file.seek(SeekFrom::Start(self.calculate_total_size() - 1))
            .await
            .map_err(|err| ApplicationDiskManagerError::DiskCreation(err.to_string()))?;
        file.write_all(&[0u8])
            .await
            .map_err(|err| ApplicationDiskManagerError::DiskCreation(err.to_string()))?;
        file.seek(SeekFrom::Start(0u64))
            .await
            .map_err(|err| ApplicationDiskManagerError::DiskCreation(err.to_string()))?;
        Ok(file.into_std().await)
    }

    fn calculate_total_size(&self) -> u64 {
        self.data_partition_size + self.image_partition_size + (34 * 2 * u64::from(Self::LBA_SIZE))
        // 68 LBA sectors for GPT
    }

    fn write_mbr(&self, mut file: impl DiskDevice) -> Result<(), ApplicationDiskManagerError> {
        let mbr = ProtectiveMBR::with_lb_size(
            (self.calculate_total_size() / u64::from(Self::LBA_SIZE) - 1) as u32,
        );
        mbr.overwrite_lba0(&mut file)
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
        mut file: impl DiskDevice,
    ) -> Result<(), ApplicationDiskManagerError> {
        self.write_mbr(&mut file)?;

        let mut gpt = GptConfig::default()
            .initialized(false)
            .writable(true)
            .logical_block_size(Self::LBA_SIZE)
            .create_from_device(Box::new(&mut file), None)
            .map_err(|err| ApplicationDiskManagerError::GPTConfiguration(err.to_string()))?;
        gpt.update_partitions(BTreeMap::new())
            .map_err(|err| ApplicationDiskManagerError::GPTConfiguration(err.to_string()))?;

        Self::create_partition(&mut gpt, Self::IMAGE_PARTITION, self.image_partition_size)?;
        Self::create_partition(&mut gpt, Self::DATA_PARTITION, self.data_partition_size)?;

        gpt.write()
            .map_err(|err| ApplicationDiskManagerError::PartitionCreation(err.to_string()))?;
        Ok(())
    }

    pub fn load_application_disk_data(
        &self,
    ) -> Result<ApplicationDiskData, ApplicationDiskManagerError> {
        self.read_partitions_uuids()?;
        self.load_paritions_uuids_from_disk()
    }
}

#[cfg(test)]
mod test {
    use std::{
        fs::{create_dir, remove_dir_all},
        path::PathBuf,
        str::FromStr,
    };

    use crate::storage::app_disk_manager::ApplicationDiskManagerError;

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

    #[tokio::test]
    #[ignore]
    async fn create_disk_and_partition() {
        let path_holder = FilePathHolder::new();
        let path = PathBuf::from_str(path_holder.disk_file_path).unwrap();
        let disk_manager = ApplicationDiskManager::new(path, 10, 10).unwrap();
        assert!(disk_manager.create_application_disk().await.is_ok());
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
