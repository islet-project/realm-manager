use crate::client_handler::realm_client_handler::RealmClientHandler;
use crate::managers::application::Application;
use crate::managers::realm::{ApplicationCreator, Realm};
use crate::managers::realm_client::RealmClient;
use crate::managers::realm_configuration::RealmConfig;
use crate::managers::realm_manager::RealmManager;
use crate::managers::warden::{RealmCreator, WardenError};
use crate::socket::vsocket_server::VSockServer;
use crate::storage::{
    create_config_path, create_workdir_path_with_uuid, read_subfolders_uuids, YamlConfigRepository,
};
use crate::utils::repository::Repository;
use crate::virtualization::qemu_runner::QemuRunner;

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::application_fabric::ApplicationFabric;

pub struct RealmManagerFabric {
    qemu_path: PathBuf,
    vsock_server: Arc<Mutex<VSockServer>>,
    warden_workdir_path: PathBuf,
    realm_connection_wait_time: Duration,
}

impl RealmManagerFabric {
    pub fn new(
        qemu_path: PathBuf,
        vsock_server: Arc<Mutex<VSockServer>>,
        warden_workdir_path: PathBuf,
        realm_connection_wait_time: Duration,
    ) -> Self {
        RealmManagerFabric {
            qemu_path,
            vsock_server,
            warden_workdir_path,
            realm_connection_wait_time,
        }
    }

    async fn load_applications(
        &self,
        realm_workdir: &Path,
        fabric: &ApplicationFabric,
        realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
    ) -> Result<HashMap<Uuid, Arc<Mutex<Box<dyn Application + Send + Sync>>>>, WardenError> {
        let mut loaded_applications = HashMap::new();
        for uuid in read_subfolders_uuids(realm_workdir)
            .await
            .map_err(|err| WardenError::RealmCreationFail(err.to_string()))?
        {
            loaded_applications.insert(
                uuid,
                Arc::new(Mutex::new(
                    fabric
                        .load_application(&uuid, realm_client_handler.clone())
                        .await
                        .map_err(|err| WardenError::RealmCreationFail(err.to_string()))?,
                )),
            );
        }
        Ok(loaded_applications)
    }
}

#[async_trait]
impl RealmCreator for RealmManagerFabric {
    async fn create_realm(
        &self,
        realm_id: Uuid,
        config: RealmConfig,
    ) -> Result<Box<dyn Realm + Send + Sync>, WardenError> {
        let realm_workdir =
            create_workdir_path_with_uuid(self.warden_workdir_path.clone(), &realm_id);
        tokio::fs::create_dir(&realm_workdir)
            .await
            .map_err(|err| WardenError::RealmCreationFail(err.to_string()))?;
        let runner = QemuRunner::new(self.qemu_path.clone(), realm_workdir.clone(), &config);
        Ok(Box::new(RealmManager::new(
            Box::new(
                YamlConfigRepository::<RealmConfig>::new(
                    config,
                    &create_config_path(realm_workdir.clone()),
                )
                .await
                .map_err(|err| WardenError::RealmCreationFail(err.to_string()))?,
            ),
            HashMap::new(),
            Box::new(runner),
            Arc::new(Mutex::new(Box::new(RealmClientHandler::new(
                self.vsock_server.clone(),
                self.realm_connection_wait_time,
            )))),
            Box::new(ApplicationFabric::new(realm_workdir)),
        )))
    }

    async fn load_realm(
        &self,
        realm_id: &Uuid,
    ) -> Result<Box<dyn Realm + Send + Sync>, WardenError> {
        let realm_workdir_path =
            create_workdir_path_with_uuid(self.warden_workdir_path.clone(), realm_id);
        let application_fabric = ApplicationFabric::new(realm_workdir_path.clone());
        let realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>> =
            Arc::new(Mutex::new(Box::new(RealmClientHandler::new(
                self.vsock_server.clone(),
                self.realm_connection_wait_time,
            ))));
        let loaded_applications = self
            .load_applications(
                &realm_workdir_path,
                &application_fabric,
                realm_client_handler.clone(),
            )
            .await?;
        let repository = YamlConfigRepository::<RealmConfig>::from(&create_config_path(
            realm_workdir_path.clone(),
        ))
        .await
        .map_err(|err| WardenError::RealmCreationFail(err.to_string()))?;
        let runner = QemuRunner::new(self.qemu_path.clone(), realm_workdir_path, repository.get());
        Ok(Box::new(RealmManager::new(
            Box::new(repository),
            loaded_applications,
            Box::new(runner),
            realm_client_handler,
            Box::new(application_fabric),
        )))
    }

    async fn clean_up_realm(&self, realm_id: &Uuid) -> Result<(), WardenError> {
        tokio::fs::remove_dir_all(create_workdir_path_with_uuid(
            self.warden_workdir_path.clone(),
            realm_id,
        ))
        .await
        .map_err(|err| WardenError::DestroyFail(err.to_string()))
    }
}
