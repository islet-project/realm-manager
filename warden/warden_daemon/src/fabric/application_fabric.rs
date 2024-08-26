use crate::{
    managers::{
        application::{Application, ApplicationConfig},
        application_manager::ApplicationManager,
        realm::{ApplicationCreator, RealmError},
        realm_client::RealmClient,
    },
    storage::{
        app_disk_manager::ApplicationDiskManager, create_config_path,
        create_workdir_path_with_uuid, YamlConfigRepository,
    },
    utils::repository::Repository,
};

use async_trait::async_trait;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct ApplicationFabric {
    realm_workdir_path: PathBuf,
}

impl ApplicationFabric {
    pub fn new(realm_workdir_path: PathBuf) -> Self {
        ApplicationFabric { realm_workdir_path }
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
        let path = create_workdir_path_with_uuid(self.realm_workdir_path.clone(), &uuid);
        tokio::fs::create_dir(&path)
            .await
            .map_err(|err| RealmError::ApplicationCreationFail(err.to_string()))?;
        let application_disk_manager = ApplicationDiskManager::new(
            path.clone(),
            config.image_storage_size_mb,
            config.data_storage_size_mb,
        )
        .map_err(|err| RealmError::ApplicationCreationFail(err.to_string()))?;
        Ok(Box::new(
            ApplicationManager::new(
                uuid,
                Box::new(
                    YamlConfigRepository::<ApplicationConfig>::new(
                        config,
                        &create_config_path(path),
                    )
                    .await
                    .map_err(|err| RealmError::ApplicationCreationFail(err.to_string()))?,
                ),
                Box::new(application_disk_manager),
                realm_client_handler,
            )
            .await
            .map_err(|err| RealmError::ApplicationCreationFail(err.to_string()))?,
        ))
    }

    async fn load_application(
        &self,
        uuid: &Uuid,
        realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
    ) -> Result<Box<dyn Application + Send + Sync>, RealmError> {
        let path = create_workdir_path_with_uuid(self.realm_workdir_path.clone(), uuid);
        let config =
            YamlConfigRepository::<ApplicationConfig>::from(&create_config_path(path.clone()))
                .await
                .map_err(|err| RealmError::ApplicationCreationFail(err.to_string()))?;
        let app_config = config.get();
        let application_disk_manager = ApplicationDiskManager::new(
            path,
            app_config.image_storage_size_mb,
            app_config.data_storage_size_mb,
        )
        .map_err(|err| RealmError::ApplicationCreationFail(err.to_string()))?;
        Ok(Box::new(
            ApplicationManager::new(
                *uuid,
                Box::new(config),
                Box::new(application_disk_manager),
                realm_client_handler,
            )
            .await
            .map_err(|err| RealmError::ApplicationCreationFail(err.to_string()))?,
        ))
    }
}
