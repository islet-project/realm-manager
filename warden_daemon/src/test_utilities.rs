use crate::client_handler::realm_client_handler::{
    RealmCommand, RealmConnector, RealmSender, RealmSenderError,
};
use crate::managers::application::ApplicationCreator;
use crate::managers::realm::RealmCreator;
use crate::managers::realm_manager::{VmManager, VmManagerError};
use crate::managers::{
    application::{Application, ApplicationConfig, ApplicationError},
    realm::{Realm, RealmData, RealmDescription, RealmError, State},
    realm_client::{RealmClient, RealmClientError, RealmProvisioningConfig},
    realm_configuration::{CpuConfig, KernelConfig, MemoryConfig, NetworkConfig, RealmConfig},
    warden::{Warden, WardenError},
};
use async_trait::async_trait;
use mockall::mock;
use std::process::ExitStatus;
use std::{path::PathBuf, str::FromStr, sync::Arc};
use tokio::sync::oneshot::Receiver;
use uuid::Uuid;

pub fn create_example_realm_data() -> RealmData {
    RealmData {
        state: State::Halted,
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
    ApplicationConfig {}
}

pub fn create_realm_provisioning_config() -> RealmProvisioningConfig {
    RealmProvisioningConfig {}
}

mock! {
    pub WardenDaemon {}

    #[async_trait]
    impl Warden for WardenDaemon {
        fn create_realm(&mut self, config: RealmConfig) -> Result<Uuid, WardenError>;
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
        fn stop(&mut self) -> Result<(), RealmError>;
        async fn reboot(&mut self) -> Result<(), RealmError>;
        fn create_application(&mut self, config: ApplicationConfig) -> Result<Uuid, RealmError>;
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
        fn update(&mut self, config: ApplicationConfig);
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
        async fn send(&mut self, data: RealmCommand) -> Result<(), RealmSenderError>;
    }
}

mock! {
    pub RealmClient {}

    #[async_trait]
    impl RealmClient for RealmClient {
        async fn send_realm_provisioning_config(
            &mut self,
            realm_provisioning_config: RealmProvisioningConfig,
            cid: u32,
        ) -> Result<(), RealmClientError>;
        async fn start_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError>;
        async fn stop_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError>;
    }
}

mock! {
    pub VmManager {}

    impl VmManager for VmManager {
        fn setup_network(&mut self, config: &NetworkConfig);
        fn setup_cpu(&mut self, config: &CpuConfig);
        fn setup_kernel(&mut self, config: &KernelConfig);
        fn setup_memory(&mut self, config: &MemoryConfig);
        fn setup_machine(&mut self, name: &str);
        fn launch_vm(&mut self) -> Result<(), VmManagerError>;
        fn stop_vm(&mut self) -> Result<(), VmManagerError>;
        fn delete_vm(&mut self) -> Result<(), VmManagerError>;
        fn get_exit_status(&mut self) -> Option<ExitStatus>;
    }
}

mock! {
    pub RealmManagerCreator {}
    impl RealmCreator for RealmManagerCreator {
        fn create_realm(&self, realm_id: Uuid, config: RealmConfig) -> Box<dyn Realm + Send + Sync>;
    }
}

mock! {
    pub ApplicationFabric {}
    impl ApplicationCreator for ApplicationFabric {
        fn create_application(&self,
            uuid: Uuid,  config: ApplicationConfig, realm_client_handler: Arc<tokio::sync::Mutex<Box<dyn RealmClient + Send + Sync>>>) -> Box<dyn Application + Send + Sync>;
    }
}
