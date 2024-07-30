use std::{
    io,
    path::{Path, PathBuf},
    str::FromStr,
};

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use utils::file_system::{fs_repository::FileRepository, workspace_manager::WorkspaceManager};
use uuid::Uuid;

use crate::utils::repository::{Repository, RepositoryError};

pub async fn read_subfolders_uuids(root_dir_path: &Path) -> Result<Vec<Uuid>, io::Error> {
    let workspace_manager = WorkspaceManager::new(root_dir_path.to_path_buf()).await?;
    let mut uuids: Vec<Uuid> = Vec::new();
    for realm_dir in workspace_manager.read_subdirectories().await?.into_iter() {
        let uuid = {
            let uuid = realm_dir.iter().last().ok_or(io::Error::other(
                "Unable to collect realm's Uuid from child dir.",
            ))?;
            Uuid::from_str(
                uuid.to_str()
                    .ok_or(io::Error::other("Failed to map OS string."))?,
            )
            .map_err(|err| io::Error::other(err.to_string()))?
        };
        uuids.push(uuid);
    }
    Ok(uuids)
}

pub fn create_config_path(mut root_path: PathBuf, realm_id: &Uuid) -> PathBuf {
    const CONFIG_FILE_NAME: &str = "config.yaml";
    root_path.push(realm_id.to_string());
    root_path.push(CONFIG_FILE_NAME);
    root_path
}

pub struct YamlConfigRepository<Config: Serialize + DeserializeOwned> {
    config: FileRepository<Config>,
}

impl<Config: Serialize + DeserializeOwned + Send + Sync> YamlConfigRepository<Config> {
    pub async fn new(config: Config, path: &Path) -> Result<Self, RepositoryError> {
        let mut yaml_repository = Self {
            config: FileRepository::<Config>::new(config, path)
                .await
                .map_err(|err| RepositoryError::CreationFail(err.to_string()))?,
        };

        yaml_repository.save().await?;

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

    use uuid::Uuid;

    #[test]
    fn create_realm_config_path() {
        let uuid = Uuid::new_v4();
        let path = super::create_config_path(PathBuf::new(), &uuid);
        assert_eq!(
            path,
            PathBuf::from_str(&format!("{}/config.yaml", uuid.to_string())).unwrap()
        );
    }
}
