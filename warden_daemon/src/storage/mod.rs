use std::{
    io,
    path::{Path, PathBuf},
    str::FromStr,
};

use async_trait::async_trait;
use utils::file_system::{fs_repository::FileRepository, workspace_manager::WorkspaceManager};
use uuid::Uuid;

use crate::managers::{
    application::{ApplicationConfig, ApplicationConfigRepository, ApplicationError},
    realm::{RealmConfigRepository, RealmError},
    realm_configuration::RealmConfig,
};

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

pub struct ApplicationConfigYamlRepository {
    config: FileRepository<ApplicationConfig>,
}

impl ApplicationConfigYamlRepository {
    pub async fn new(config: ApplicationConfig, path: &Path) -> Result<Self, String> {
        Ok(Self {
            config: FileRepository::<ApplicationConfig>::new(config, path)
                .await
                .map_err(|err| err.to_string())?,
        })
    }

    pub fn from(repository: FileRepository<ApplicationConfig>) -> Self {
        Self { config: repository }
    }
}

#[async_trait]
impl ApplicationConfigRepository for ApplicationConfigYamlRepository {
    fn get_application_config(&mut self) -> &mut ApplicationConfig {
        self.config.get()
    }

    async fn save_realm_config(&mut self) -> Result<(), ApplicationError> {
        self.config
            .save()
            .await
            .map_err(|err| ApplicationError::StorageOperationFail(err.to_string()))
    }
}

pub struct RealmConfigYamlRepository {
    config: FileRepository<RealmConfig>,
}

impl RealmConfigYamlRepository {
    pub async fn new(config: RealmConfig, path: &Path) -> Result<Self, String> {
        Ok(Self {
            config: FileRepository::<RealmConfig>::new(config, path)
                .await
                .map_err(|err| err.to_string())?,
        })
    }

    pub fn from(repository: FileRepository<RealmConfig>) -> Self {
        Self { config: repository }
    }
}

#[async_trait]
impl RealmConfigRepository for RealmConfigYamlRepository {
    fn get_realm_config(&mut self) -> &mut RealmConfig {
        self.config.get()
    }

    async fn save_realm_config(&mut self) -> Result<(), RealmError> {
        self.config
            .save()
            .await
            .map_err(|err| RealmError::StorageOperationFail(err.to_string()))
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
