use crate::client_handler::realm_client_handler::RealmClientHandler;
use crate::managers::application::Application;
use crate::managers::realm::{ApplicationCreator, Realm};
use crate::managers::realm_client::RealmClient;
use crate::managers::realm_configuration::RealmConfig;
use crate::managers::realm_manager::RealmManager;
use crate::managers::vm_manager::{VmManager, VmManagerError};
use crate::managers::warden::{RealmCreator, WardenError};
use crate::socket::vsocket_server::VSockServer;
use crate::storage::{
    create_config_path, create_workdir_path_with_uuid, read_subfolders_uuids, YamlConfigRepository,
};
use crate::utils::repository::Repository;
use crate::virtualization::network::NetworkManager;

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::application_fabric::ApplicationFabric;

type Creator = Box<
    dyn Fn(PathBuf, &RealmConfig) -> Result<Box<dyn VmManager + Send + Sync>, VmManagerError>
        + Send
        + Sync,
>;

pub struct RealmManagerFabric<N: NetworkManager + Send + Sync> {
    vm_manager_creator: Creator,
    vsock_server: Arc<Mutex<VSockServer>>,
    network_manager: Arc<Mutex<N>>,
    warden_workdir_path: PathBuf,
    realm_connection_wait_time: Duration,
    realm_response_wait_time: Duration,
}

impl<N: NetworkManager + Send + Sync + 'static> RealmManagerFabric<N> {
    pub fn new(
        vm_manager_creator: Creator,
        vsock_server: Arc<Mutex<VSockServer>>,
        warden_workdir_path: PathBuf,
        network_manager: Arc<Mutex<N>>,
        realm_connection_wait_time: Duration,
        realm_response_wait_time: Duration,
    ) -> Self {
        RealmManagerFabric::<N> {
            vm_manager_creator,
            vsock_server,
            network_manager,
            warden_workdir_path,
            realm_connection_wait_time,
            realm_response_wait_time,
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
impl<N: NetworkManager + Send + Sync + 'static> RealmCreator for RealmManagerFabric<N> {
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
        self.network_manager
            .lock()
            .await
            .create_tap_device_for_realm(config.network.tap_device.clone(), realm_id)
            .await
            .map_err(|err| WardenError::RealmCreationFail(err.to_string()))?;
        let vm_manager = self.vm_manager_creator.as_ref()(realm_workdir.clone(), &config)
            .map_err(|err| WardenError::RealmCreationFail(err.to_string()))?;
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
            vm_manager,
            Arc::new(Mutex::new(Box::new(RealmClientHandler::new(
                self.vsock_server.clone(),
                self.realm_connection_wait_time,
                self.realm_response_wait_time,
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
                self.realm_response_wait_time,
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
        self.network_manager
            .lock()
            .await
            .create_tap_device_for_realm(repository.get().network.tap_device.clone(), *realm_id)
            .await
            .map_err(|err| WardenError::RealmCreationFail(err.to_string()))?;
        let vm_manager = self.vm_manager_creator.as_ref()(realm_workdir_path, repository.get())
            .map_err(|err| WardenError::RealmCreationFail(err.to_string()))?;
        Ok(Box::new(RealmManager::new(
            Box::new(repository),
            loaded_applications,
            vm_manager,
            realm_client_handler,
            Box::new(application_fabric),
        )))
    }

    async fn clean_up_realm(&self, realm_id: &Uuid) -> Result<(), WardenError> {
        self.network_manager
            .lock()
            .await
            .shutdown_tap_device_for_realm(*realm_id)
            .await
            .map_err(|err| WardenError::DestroyFail(err.to_string()))?;
        tokio::fs::remove_dir_all(create_workdir_path_with_uuid(
            self.warden_workdir_path.clone(),
            realm_id,
        ))
        .await
        .map_err(|err| WardenError::DestroyFail(err.to_string()))
    }
}
