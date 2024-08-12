use crate::{
    managers::{
        application::{Application, ApplicationConfig},
        application_manager::ApplicationManager,
        realm::{ApplicationCreator, RealmError},
        realm_client::RealmClient,
    },
    storage::{create_config_path, create_workdir_path_with_uuid, YamlConfigRepository},
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
        Ok(Box::new(ApplicationManager::new(
            uuid,
            Box::new(
                YamlConfigRepository::<ApplicationConfig>::new(config, &create_config_path(path))
                    .await
                    .map_err(|err| RealmError::ApplicationCreationFail(err.to_string()))?,
            ),
            realm_client_handler,
        )))
    }

    async fn load_application(
        &self,
        uuid: &Uuid,
        realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
    ) -> Result<Box<dyn Application + Send + Sync>, RealmError> {
        let path = create_workdir_path_with_uuid(self.realm_workdir_path.clone(), uuid);
        Ok(Box::new(ApplicationManager::new(
            *uuid,
            Box::new(
                YamlConfigRepository::<ApplicationConfig>::from(&create_config_path(path))
                    .await
                    .map_err(|err| RealmError::ApplicationCreationFail(err.to_string()))?,
            ),
            realm_client_handler,
        )))
    }
}
