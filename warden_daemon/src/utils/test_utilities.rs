use super::repository::{Repository, RepositoryError};
use crate::client_handler::realm_client_handler::{RealmConnector, RealmSender, RealmSenderError};
use crate::managers::application::ApplicationData;
use crate::managers::realm_manager::{VmManager, VmManagerError};
use crate::managers::{
    application::{Application, ApplicationConfig, ApplicationError},
    realm::{ApplicationCreator, Realm, RealmData, RealmDescription, RealmError, State},
    realm_client::{RealmClient, RealmClientError, RealmProvisioningConfig},
    realm_configuration::{CpuConfig, KernelConfig, MemoryConfig, NetworkConfig, RealmConfig},
    warden::{RealmCreator, Warden, WardenError},
};
use async_trait::async_trait;
use mockall::mock;
use std::process::ExitStatus;
use std::time::Duration;
use std::{path::PathBuf, str::FromStr, sync::Arc};
use tokio::sync::oneshot::Receiver;
use uuid::Uuid;
use warden_realm::{Request, Response};

pub fn create_example_realm_data() -> RealmData {
    RealmData {
        state: State::Halted,
        applications: vec![],
    }
}

pub fn create_example_realm_description() -> RealmDescription {
    RealmDescription {
        uuid: create_example_uuid(),
        realm_data: create_example_realm_data(),
    }
}

pub fn create_example_uuid() -> Uuid {
    Uuid::from_str("a46289a4-5902-4586-81a3-908bdd62e7a1").unwrap()
}

pub fn create_example_realm_config() -> RealmConfig {
    RealmConfig {
        machine: String::new(),
        cpu: CpuConfig {
            cpu: String::new(),
            cores_number: 0,
        },
        memory: MemoryConfig { ram_size: 0 },
        network: NetworkConfig {
            vsock_cid: 0,
            tap_device: String::new(),
            mac_address: String::new(),
            hardware_device: None,
            remote_terminal_uri: None,
        },
        kernel: KernelConfig {
            kernel_path: PathBuf::new(),
        },
    }
}

pub fn create_example_app_config() -> ApplicationConfig {
    ApplicationConfig {
        name: String::new(),
        version: String::new(),
        image_registry: String::new(),
        image_storage_size_mb: 0,
        data_storage_size_mb: 0,
    }
}

pub fn create_realm_provisioning_config() -> RealmProvisioningConfig {
    RealmProvisioningConfig {
        applications_data: vec![],
    }
}

mock! {
    pub WardenDaemon {}

    #[async_trait]
    impl Warden for WardenDaemon {
        async fn create_realm(&mut self, config: RealmConfig) -> Result<Uuid, WardenError>;
        async fn destroy_realm(&mut self, realm_uuid:& Uuid) -> Result<(), WardenError>;
        async fn list_realms(&self) -> Vec<RealmDescription>;
        async fn inspect_realm(&self, realm_uuid:& Uuid) -> Result<RealmDescription, WardenError>;
        fn get_realm(
            &mut self,
            realm_uuid: &Uuid,
        ) -> Result<Arc<tokio::sync::Mutex<Box<dyn Realm + Send + Sync>>>, WardenError>;
    }
}

mock! {
    pub Realm{}
    #[async_trait]
    impl Realm for Realm {
        async fn start(&mut self) -> Result<(), RealmError>;
        async fn stop(&mut self) -> Result<(), RealmError>;
        async fn reboot(&mut self) -> Result<(), RealmError>;
        async fn create_application(&mut self, config: ApplicationConfig) -> Result<Uuid, RealmError>;
        fn get_realm_data(& self) -> RealmData;
        fn get_application(&self, uuid:& Uuid) -> Result<Arc<tokio::sync::Mutex<Box<dyn Application + Send + Sync>>>, RealmError>;
        async fn update_application(&mut self, uuid:& Uuid, new_config: ApplicationConfig) -> Result<(), RealmError>;
    }
}

mock! {
    pub Application {}
    #[async_trait]
    impl Application for Application {
        async fn stop(&mut self) -> Result<(), ApplicationError>;
        async fn start(&mut self) -> Result<(), ApplicationError>;
        fn get_data(&self) -> ApplicationData;
        async fn update(&mut self, config: ApplicationConfig) -> Result<(), ApplicationError>;
    }
}

mock! {
    pub RealmConnector {}

    #[async_trait]
    impl RealmConnector for RealmConnector {
        async fn acquire_realm_sender(
            &mut self,
            cid: u32,
        ) -> Receiver<Box<dyn RealmSender + Send + Sync>>;
    }
}

mock! {
    pub RealmSender {}

    #[async_trait]
    impl RealmSender for RealmSender {
        async fn send(&mut self, data: Request) -> Result<(), RealmSenderError>;
        async fn receive_response(&mut self, timeout: Duration) -> Result<Response, RealmSenderError>;
    }
}

mock! {
    pub RealmClient {}

    #[async_trait]
    impl RealmClient for RealmClient {
        async fn provision_applications(
            &mut self,
            realm_provisioning_config: RealmProvisioningConfig,
            cid: u32,
        ) -> Result<(), RealmClientError>;
        async fn start_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError>;
        async fn stop_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError>;
        async fn kill_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError>;
        async fn shutdown_realm(&mut self) -> Result<(), RealmClientError>;
        async fn reboot_realm(&mut self,
            realm_provisioning_config: RealmProvisioningConfig,
            cid: u32,) -> Result<(), RealmClientError>;
    }
}

mock! {
    pub VmManager {}

    impl VmManager for VmManager {
        fn launch_vm(&mut self) -> Result<(), VmManagerError>;
        fn stop_vm(&mut self) -> Result<(), VmManagerError>;
        fn delete_vm(&mut self) -> Result<(), VmManagerError>;
        fn get_exit_status(&mut self) -> Option<ExitStatus>;
    }
}

mock! {
    pub RealmManagerCreator {}
    #[async_trait]
    impl RealmCreator for RealmManagerCreator {
        async fn create_realm(&self, realm_id: Uuid, config: RealmConfig) -> Result<Box<dyn Realm + Send + Sync>, WardenError>;
        async fn load_realm(&self, realm_id:& Uuid) -> Result<Box<dyn Realm + Send + Sync>, WardenError>;
        async fn clean_up_realm(&self, realm_id: &Uuid) -> Result<(), WardenError>;
    }
}

mock! {
    pub ApplicationFabric {}
    #[async_trait]
    impl ApplicationCreator for ApplicationFabric {
        async fn create_application(
            &self,
            uuid: Uuid,
            config: ApplicationConfig,
            realm_client_handler: Arc<tokio::sync::Mutex<Box<dyn RealmClient + Send + Sync>>>,
        ) -> Result<Box<dyn Application + Send + Sync>, RealmError>;
        async fn load_application(
            &self,
            realm_id: &Uuid,
            realm_client_handler: Arc<tokio::sync::Mutex<Box<dyn RealmClient + Send + Sync>>>
        ) -> Result<Box<dyn Application + Send + Sync>, RealmError>;
        }
}

mock! {
    pub ApplicationRepository {}

    #[async_trait]
    impl Repository for ApplicationRepository {
        type Data = ApplicationConfig;
        fn get(&self) -> &ApplicationConfig;
        fn get_mut(&mut self) -> &mut ApplicationConfig;
        async fn save(&mut self) -> Result<(), RepositoryError>;
    }
}

mock! {
    pub RealmRepository {}

    #[async_trait]
    impl Repository for RealmRepository {
        type Data = RealmConfig;
        fn get(&self) -> &RealmConfig;
        fn get_mut(&mut self) -> &mut RealmConfig;
        async fn save(&mut self) -> Result<(), RepositoryError>;
    }
}