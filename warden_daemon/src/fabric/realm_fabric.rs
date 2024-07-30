use crate::client_handler::realm_client_handler::RealmClientHandler;
use crate::managers::application::Application;
use crate::managers::realm::{ApplicationCreator, Realm};
use crate::managers::realm_client::RealmClient;
use crate::managers::realm_configuration::RealmConfig;
use crate::managers::realm_manager::RealmManager;
use crate::managers::warden::{RealmCreator, WardenError};
use crate::socket::vsocket_server::VSockServer;
use crate::storage::{create_config_path, read_subfolders_uuids, YamlConfigRepository};
use crate::virtualization::qemu_runner::QemuRunner;

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use utils::file_system::workspace_manager::WorkspaceManager;
use uuid::Uuid;

use super::application_fabric::ApplicationFabric;

pub struct RealmManagerFabric {
    qemu_path: PathBuf,
    vsock_server: Arc<Mutex<VSockServer>>,
    warden_workdir_path: PathBuf,
}

impl RealmManagerFabric {
    pub fn new(
        qemu_path: PathBuf,
        vsock_server: Arc<Mutex<VSockServer>>,
        warden_workdir_path: PathBuf,
    ) -> Self {
        RealmManagerFabric {
            qemu_path,
            vsock_server,
            warden_workdir_path,
        }
    }

    fn create_application_fabric(&self, realm_id: &Uuid) -> ApplicationFabric {
        ApplicationFabric::new(self.create_realm_workdir_path(realm_id))
    }

    fn create_realm_workdir_path(&self, realm_id: &Uuid) -> PathBuf {
        let mut path = self.warden_workdir_path.clone();
        path.push(realm_id.to_string());
        path
    }

    async fn load_applications(
        &self,
        realm_id: &Uuid,
        fabric: &ApplicationFabric,
        realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
    ) -> Result<HashMap<Uuid, Arc<Mutex<Box<dyn Application + Send + Sync>>>>, WardenError> {
        let mut loaded_applications = HashMap::new();
        for uuid in read_subfolders_uuids(&self.create_realm_workdir_path(realm_id))
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
        let path = create_config_path(self.warden_workdir_path.clone(), &realm_id);
        Ok(Box::new(RealmManager::new(
            Box::new(
                YamlConfigRepository::<RealmConfig>::new(config, &path)
                    .await
                    .map_err(|err| WardenError::RealmCreationFail(err.to_string()))?,
            ),
            HashMap::new(),
            Box::new(QemuRunner::new(self.qemu_path.clone())),
            Arc::new(Mutex::new(Box::new(RealmClientHandler::new(
                self.vsock_server.clone(),
            )))),
            Box::new(self.create_application_fabric(&realm_id)),
        )))
    }

    async fn load_realm(
        &self,
        realm_id: &Uuid,
    ) -> Result<Box<dyn Realm + Send + Sync>, WardenError> {
        let realm_config_folder_path =
            create_config_path(self.warden_workdir_path.clone(), realm_id);
        let application_fabric = self.create_application_fabric(realm_id);
        let realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>> = Arc::new(
            Mutex::new(Box::new(RealmClientHandler::new(self.vsock_server.clone()))),
        );
        let loaded_applications = self
            .load_applications(realm_id, &application_fabric, realm_client_handler.clone())
            .await?;
        Ok(Box::new(RealmManager::new(
            Box::new(
                YamlConfigRepository::<RealmConfig>::from(&realm_config_folder_path)
                    .await
                    .map_err(|err| WardenError::RealmCreationFail(err.to_string()))?,
            ),
            loaded_applications,
            Box::new(QemuRunner::new(self.qemu_path.clone())),
            realm_client_handler,
            Box::new(application_fabric),
        )))
    }

    async fn clean_up_realm(&self, realm_id: &Uuid) -> Result<(), WardenError> {
        let workspace_manager = WorkspaceManager::new(self.create_realm_workdir_path(realm_id))
            .await
            .map_err(|err| WardenError::DestroyFail(err.to_string()))?;
        workspace_manager
            .destroy_workspace()
            .await
            .map_err(|err| WardenError::DestroyFail(err.to_string()))
    }
}
