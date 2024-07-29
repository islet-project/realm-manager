use crate::{
    managers::{
        application::{Application, ApplicationConfig, ApplicationConfigRepository},
        application_manager::ApplicationManager,
        realm::{ApplicationCreator, RealmError},
        realm_client::RealmClient,
    },
    storage::{create_config_path, ApplicationConfigYamlRepository},
};

use async_trait::async_trait;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::Mutex;
use utils::file_system::fs_repository::FileRepository;
use uuid::Uuid;

pub struct ApplicationFabric {
    realm_workdir_path: PathBuf,
}

impl ApplicationFabric {
    pub fn new(realm_workdir_path: PathBuf) -> Self {
        ApplicationFabric { realm_workdir_path }
    }

    async fn create_application_config_repository(
        config: ApplicationConfig,
        path: &Path,
    ) -> Result<ApplicationConfigYamlRepository, RealmError> {
        let mut app_config_repository = ApplicationConfigYamlRepository::new(config, path)
            .await
            .map_err(|err| RealmError::ApplicationCreationFail(err.to_string()))?;
        app_config_repository
            .save_realm_config()
            .await
            .map_err(|err| RealmError::ApplicationCreationFail(err.to_string()))?;
        Ok(app_config_repository)
    }

    async fn load_application_config(
        config_path: &Path,
    ) -> Result<ApplicationConfigYamlRepository, RealmError> {
        Ok(ApplicationConfigYamlRepository::from(
            FileRepository::<ApplicationConfig>::from_file_path(config_path)
                .await
                .map_err(|err| RealmError::ApplicationCreationFail(err.to_string()))?,
        ))
    }
}

#[async_trait]
impl ApplicationCreator for ApplicationFabric {
    async fn create_application(
        &self,
        uuid: Uuid,
        config: ApplicationConfig,
        realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
    ) -> Result<Box<dyn Application + Send + Sync>, RealmError> {
        let path = create_config_path(self.realm_workdir_path.clone(), &uuid);
        Ok(Box::new(ApplicationManager::new(
            uuid,
            Box::new(ApplicationFabric::create_application_config_repository(config, &path).await?),
            realm_client_handler,
        )))
    }

    async fn load_application(
        &self,
        realm_id: &Uuid,
        realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
    ) -> Result<Box<dyn Application + Send + Sync>, RealmError> {
        let application_config_path = create_config_path(self.realm_workdir_path.clone(), realm_id);
        Ok(Box::new(ApplicationManager::new(
            *realm_id,
            Box::new(ApplicationFabric::load_application_config(&application_config_path).await?),
            realm_client_handler,
        )))
    }
}
