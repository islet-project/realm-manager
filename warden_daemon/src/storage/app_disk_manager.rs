use std::{
    collections::{BTreeMap, HashMap},
    io::SeekFrom,
    path::PathBuf,
};

use gpt::{mbr::ProtectiveMBR, GptConfig};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{
    fs::File,
    io::{AsyncSeekExt, AsyncWriteExt},
};
use uuid::Uuid;

use crate::managers::application::ApplicationDisk;

#[derive(Error, Debug, PartialEq, PartialOrd, Clone, Serialize, Deserialize)]
pub enum ApplicationDiskManagerError {
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

    pub fn new(
        mut workdir_path: PathBuf,
        image_partition_size_mb: u32,
        data_partition_size_mb: u32,
    ) -> Self {
        let data_partition_size: u64 = (data_partition_size_mb * 1024 * 1024).into();
        let image_partition_size: u64 = (image_partition_size_mb * 1024 * 1024).into();
        workdir_path.push(Self::DISK_NAME);
        Self {
            file_path: workdir_path,
            image_partition_size,
            data_partition_size,
        }
    }

    pub async fn create_application_disk(
        &self,
    ) -> Result<ApplicationDisk, ApplicationDiskManagerError> {
        let file = self.create_file().await?.into_std().await;
        self.create_gpt_and_partitions(&file)?;
        Self::sync_file(file).await?;

        self.handle_partitions_load()
    }

    fn handle_partitions_load(&self) -> Result<ApplicationDisk, ApplicationDiskManagerError> {
        let partitions_data = self.read_partitions_uuids()?;
        Ok(ApplicationDisk {
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

    async fn create_file(&self) -> Result<File, ApplicationDiskManagerError> {
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
        Ok(file)
    }

    fn calculate_total_size(&self) -> u64 {
        self.data_partition_size + self.image_partition_size + (34 * 2 * u64::from(Self::LBA_SIZE))
        // 68 LBA sectors for GPT
    }

    fn create_gpt_and_partitions(
        &self,
        mut file: &std::fs::File,
    ) -> Result<(), ApplicationDiskManagerError> {
        let mbr = ProtectiveMBR::with_lb_size(
            (self.calculate_total_size() / u64::from(Self::LBA_SIZE) - 1) as u32,
        );
        mbr.overwrite_lba0(&mut file)
            .map_err(|err| ApplicationDiskManagerError::PartitionMetadataWrite(err.to_string()))?;
        let mut gpt = GptConfig::default()
            .initialized(false)
            .writable(true)
            .logical_block_size(Self::LBA_SIZE)
            .create_from_device(Box::new(&mut file), None)
            .map_err(|err| ApplicationDiskManagerError::GPTConfiguration(err.to_string()))?;
        gpt.update_partitions(BTreeMap::new())
            .map_err(|err| ApplicationDiskManagerError::GPTConfiguration(err.to_string()))?;

        gpt.add_partition(
            Self::IMAGE_PARTITION,
            self.image_partition_size,
            gpt::partition_types::LINUX_FS,
            0,
            None,
        )
        .map_err(|err| ApplicationDiskManagerError::PartitionCreation(err.to_string()))?;

        gpt.add_partition(
            Self::DATA_PARTITION,
            self.data_partition_size,
            gpt::partition_types::LINUX_FS,
            0,
            None,
        )
        .map_err(|err| ApplicationDiskManagerError::PartitionCreation(err.to_string()))?;

        gpt.write()
            .map_err(|err| ApplicationDiskManagerError::PartitionCreation(err.to_string()))?;
        Ok(())
    }

    pub fn load_application_disk_data(
        &self,
    ) -> Result<ApplicationDisk, ApplicationDiskManagerError> {
        self.read_partitions_uuids()?;
        self.handle_partitions_load()
    }
}

#[cfg(test)]
mod test {
    use std::{
        fs::{create_dir, remove_dir_all},
        path::PathBuf,
        str::FromStr,
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

    #[tokio::test]
    #[ignore]
    async fn create_disk_and_partition() {
        let path_holder = FilePathHolder::new();
        let path = PathBuf::from_str(path_holder.disk_file_path).unwrap();
        let disk_manager = ApplicationDiskManager::new(path, 10, 10);
        assert!(disk_manager.create_application_disk().await.is_ok());
    }
}
