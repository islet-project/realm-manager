use std::path::Path;

use async_trait::async_trait;
use utils::file_system::fs_repository::FileRepository;

use crate::managers::{
    realm::{RealmConfigRepository, RealmError},
    realm_configuration::RealmConfig,
};

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
