use std::{
    io,
    path::{Path, PathBuf},
    str::FromStr,
};

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;
use utils::file_system::fs_repository::FileRepository;
use uuid::Uuid;

use crate::{
    managers::application::ApplicationDisk,
    utils::repository::{Repository, RepositoryError},
};

pub async fn read_subfolders_uuids(root_folder: &Path) -> Result<Vec<Uuid>, io::Error> {
    let mut uuids: Vec<Uuid> = Vec::new();
    let mut read_dir = tokio::fs::read_dir(root_folder).await?;
    while let Ok(Some(entry)) = read_dir.next_entry().await {
        if let Ok(file_type) = entry.file_type().await {
            if file_type.is_dir() {
                uuids.push(
                    Uuid::from_str(entry.file_name().to_string_lossy().as_ref())
                        .map_err(|err| io::Error::other(err.to_string()))?,
                );
            }
        }
    }
    Ok(uuids)
}

#[derive(Error, Debug, PartialEq, PartialOrd, Clone, Serialize, Deserialize)]
pub enum ApplicationDiskManagerError {}

pub struct ApplicationDiskManager;

impl ApplicationDiskManager {
    pub fn create_application_disk(
        workdir_path: &Path,
        image_partition_size: u32,
        data_partition_size: u32,
    ) -> Result<ApplicationDisk, ApplicationDiskManagerError> {
        Ok(ApplicationDisk { image_partition_uuid: Uuid::new_v4(), data_partition_uuid: Uuid::new_v4() })
    }

    pub fn load_application_disk_data(
        workdir_path: &Path,
    ) -> Result<ApplicationDisk, ApplicationDiskManagerError> {
        Ok(ApplicationDisk { image_partition_uuid: Uuid::new_v4(), data_partition_uuid: Uuid::new_v4() })
    }
}

pub fn create_config_path(mut root_path: PathBuf) -> PathBuf {
    const CONFIG_FILE_NAME: &str = "config.yaml";
    root_path.push(CONFIG_FILE_NAME);
    root_path
}

pub fn create_workdir_path_with_uuid(mut root_workdir: PathBuf, realm_id: &Uuid) -> PathBuf {
    root_workdir.push(realm_id.to_string());
    root_workdir
}

pub struct YamlConfigRepository<Config: Serialize + DeserializeOwned> {
    config: FileRepository<Config>,
}

impl<Config: Serialize + DeserializeOwned + Send + Sync> YamlConfigRepository<Config> {
    pub async fn new(config: Config, path: &Path) -> Result<Self, RepositoryError> {
        let yaml_repository = Self {
            config: FileRepository::<Config>::new(config, path)
                .await
                .map_err(|err| RepositoryError::CreationFail(err.to_string()))?,
        };
        Ok(yaml_repository)
    }

    pub async fn from(config_path: &Path) -> Result<Self, RepositoryError> {
        let file_repository = FileRepository::<Config>::from_file_path(config_path)
            .await
            .map_err(|err| RepositoryError::CreationFail(err.to_string()))?;
        Ok(Self {
            config: file_repository,
        })
    }
}

#[async_trait]
impl<Config: Serialize + DeserializeOwned + Send + Sync> Repository
    for YamlConfigRepository<Config>
{
    type Data = Config;

    fn get(&self) -> &Self::Data {
        self.config.get()
    }
    fn get_mut(&mut self) -> &mut Self::Data {
        self.config.get_mut()
    }
    async fn save(&mut self) -> Result<(), RepositoryError> {
        self.config
            .save()
            .await
            .map_err(|err| RepositoryError::SaveFail(err.to_string()))
    }
}

#[cfg(test)]
mod test {
    use std::{path::PathBuf, str::FromStr};

    #[test]
    fn create_realm_config_path() {
        let path = super::create_config_path(PathBuf::new());
        assert_eq!(path, PathBuf::from_str("config.yaml").unwrap());
    }
}
