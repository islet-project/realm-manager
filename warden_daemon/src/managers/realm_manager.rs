use async_trait::async_trait;
use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::application::{Application, ApplicationConfig, ApplicationCreator};
use super::realm::{Realm, RealmData, RealmError};
use super::realm_configuration::*;

#[derive(Debug, PartialEq, PartialOrd)]
enum State {
    Halted,
    Provisioning,
    Running,
    NeedReboot,
}

pub trait VmManager {
    fn setup_network(&mut self, config: &NetworkConfig);
    fn setup_cpu(&mut self, config: &CpuConfig);
    fn setup_disc(&mut self, config: &DiscConfig);
    fn setup_memory(&mut self, config: &MemoryConfig);
    fn setup_machine(&mut self, name: &String);
    fn launch_vm(&mut self, config: &KernelConfig) -> Result<(), VmManagerError>;
    fn stop_vm(&mut self) -> Result<(), VmManagerError>;
    fn delete_vm(&self);
}

#[derive(Debug, Error)]
pub enum VmManagerError {
    #[error("")]
    LaunchFail(#[from] io::Error),
    #[error("To stop realm's vm you need to launch it first!")]
    VmNotLaunched,
    #[error("Unalbe to stop realm's vm")]
    StopFail,
}

pub enum RealmClientError {
    RealmConnectorError,
    NoConnectionWithRealm,
}

#[async_trait]
pub trait RealmClient {
    async fn acknowledge_client_connection(&mut self, cid: u32) -> Result<(), RealmClientError>;
}

pub struct RealmManager {
    state: State,
    config: RealmConfig,
    managers_map: HashMap<Uuid, Box<dyn Application + Send + Sync>>,
    vm_manager: Box<dyn VmManager + Send + Sync>,
    realm_client: Box<dyn RealmClient + Send + Sync>,
    application_fabric: Arc<Box<dyn ApplicationCreator + Send + Sync>>,
}

impl RealmManager {
    pub fn new(
        config: RealmConfig,
        vm_manager: Box<dyn VmManager + Send + Sync>,
        realm_client: Box<dyn RealmClient + Send + Sync>,
        application_fabric: Arc<Box<dyn ApplicationCreator + Send + Sync>>,
    ) -> Self {
        RealmManager {
            state: State::Halted,
            managers_map: HashMap::new(),
            config,
            vm_manager,
            realm_client,
            application_fabric,
        }
    }
}

#[async_trait]
impl Realm for RealmManager {
    async fn start(&mut self) -> Result<(), RealmError> {
        if self.state != State::Halted {
            return Err(RealmError::UnsupportedAction(String::from(
                "Can't start realm that is not halted",
            )));
        }

        self.setup_vm()?;

        self.state = State::Provisioning;

        match self
            .realm_client
            .acknowledge_client_connection(self.config.network.vsock_cid)
            .await
        {
            Ok(_) => {
                self.state = State::Running;
                Ok(())
            }
            Err(_) => {
                self.state = State::Halted;
                Err(RealmError::RealmCantStart)
            }
        }
    }

    fn stop(&mut self) {
        todo!()
    }

    fn reboot(&mut self) {
        todo!()
    }

    fn create_application(&mut self, config: ApplicationConfig) -> Uuid {
        let uuid = Uuid::new_v4();
        let _ = self
            .managers_map
            .insert(uuid, self.application_fabric.create_application(config));
        uuid
    }

    async fn get_application(
        &self,
        _uuid: &Uuid,
    ) -> Result<Arc<Mutex<Box<dyn Application + Send + Sync>>>, RealmError> {
        todo!()
    }

    fn get_realm_data(&self) -> RealmData {
        RealmData {}
    }
}

impl RealmManager {
    fn setup_vm(&mut self) -> Result<(), RealmError> {
        self.vm_manager.setup_cpu(&self.config.cpu);
        self.vm_manager.setup_disc(&self.config.disc);
        self.vm_manager.setup_memory(&self.config.memory);
        self.vm_manager.setup_machine(&self.config.machine);
        self.vm_manager.setup_network(&self.config.network);
        self.vm_manager
            .launch_vm(&self.config.kernel)
            .map_err(|err| RealmError::RealmLaunchFail(err.to_string()))
    }
}

#[cfg(test)]
mod test {
    use std::{io::Error, sync::Arc};

    use async_trait::async_trait;
    use mockall::mock;
    use parameterized::parameterized;

    use super::{
        RealmClient, RealmClientError, RealmConfig, RealmError, RealmManager, VmManager,
        VmManagerError,
    };
    use crate::managers::{
        application::{Application, ApplicationConfig, ApplicationCreator, ApplicationError},
        realm::Realm,
        realm_configuration::*,
        realm_manager::State,
    };

    #[test]
    fn initial_state_is_halted() {
        let realm_manager = create_realm_manager(create_example_config(), None, None);
        assert_eq!(realm_manager.state, State::Halted);
    }

    #[tokio::test]
    async fn realm_start() {
        let mut realm_manager = create_realm_manager(create_example_config(), None, None);
        assert_eq!(realm_manager.start().await, Ok(()));
        assert_eq!(realm_manager.state, State::Running);
    }

    #[tokio::test]
    #[parameterized(state = {State::Provisioning, State::Running, State::NeedReboot})]
    async fn realm_start_invalid_action(state: State) {
        let mut realm_manager = create_realm_manager(create_example_config(), None, None);
        realm_manager.state = state;
        assert_eq!(
            realm_manager.start().await,
            Err(RealmError::UnsupportedAction(String::from(
                "Can't start realm that is not halted"
            )))
        );
    }

    #[tokio::test]
    async fn realm_start_launching_vm_error() {
        let mut vm_manager_mock = MockVmManager::new();
        vm_manager_mock.expect_setup_cpu().returning(|_| ());
        vm_manager_mock.expect_setup_disc().returning(|_| ());
        vm_manager_mock.expect_setup_machine().returning(|_| ());
        vm_manager_mock.expect_setup_memory().returning(|_| ());
        vm_manager_mock.expect_setup_network().returning(|_| ());
        vm_manager_mock
            .expect_launch_vm()
            .returning(|_| Err(VmManagerError::LaunchFail(Error::other(""))));
        let mut realm_manager =
            create_realm_manager(create_example_config(), Some(vm_manager_mock), None);
        assert_eq!(
            realm_manager.start().await,
            Err(RealmError::RealmLaunchFail(String::new()))
        );
        assert_eq!(realm_manager.state, State::Halted);
    }

    #[tokio::test]
    async fn realm_start_launching_acknowledgment_error() {
        let mut client_mock = MockRealmClient::new();
        client_mock
            .expect_acknowledge_client_connection()
            .returning(|_| Err(RealmClientError::RealmConnectorError));
        let mut realm_manager =
            create_realm_manager(create_example_config(), None, Some(client_mock));
        assert_eq!(realm_manager.start().await, Err(RealmError::RealmCantStart));
        assert_eq!(realm_manager.state, State::Halted);
    }

    #[test]
    fn create_application() {
        let mut realm_manager = create_realm_manager(create_example_config(), None, None);
        let uuid = realm_manager.create_application(ApplicationConfig {});
        assert!(realm_manager.managers_map.contains_key(&uuid));
    }

    fn create_realm_manager(
        config: RealmConfig,
        vm_manager: Option<MockVmManager>,
        realm_client: Option<MockRealmClient>,
    ) -> RealmManager {
        let mut vm_manager = vm_manager.unwrap_or(MockVmManager::new());
        vm_manager.expect_setup_cpu().returning(|_| ());
        vm_manager.expect_setup_disc().returning(|_| ());
        vm_manager.expect_setup_machine().returning(|_| ());
        vm_manager.expect_setup_memory().returning(|_| ());
        vm_manager.expect_setup_network().returning(|_| ());
        vm_manager.expect_launch_vm().returning(|_| Ok(()));
        let mut realm_client = realm_client.unwrap_or(MockRealmClient::new());
        realm_client
            .expect_acknowledge_client_connection()
            .returning(|_| Ok(()));
        let app_mock = MockApplication::new();
        let mut creator_mock = MockApplicationFabric::new();
        creator_mock
            .expect_create_application()
            .return_once(move |_| Box::new(app_mock));
        RealmManager::new(
            config,
            Box::new(vm_manager),
            Box::new(realm_client),
            Arc::new(Box::new(creator_mock)),
        )
    }

    fn create_example_config() -> RealmConfig {
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
                hardware_device: None,
            },
            disc: DiscConfig {
                drive: None,
                drive_format: None,
            },
            kernel: KernelConfig {
                kernel_path: String::new(),
            },
        }
    }

    mock! {
        pub VmManager {}

        impl VmManager for VmManager {
            fn setup_network(&mut self, config: &NetworkConfig);
            fn setup_cpu(&mut self, config: &CpuConfig);
            fn setup_disc(&mut self, config: &DiscConfig);
            fn setup_memory(&mut self, config: &MemoryConfig);
            fn setup_machine(&mut self, name: &String);
            fn launch_vm(&mut self, config: &KernelConfig) -> Result<(), VmManagerError>;
            fn stop_vm(&mut self) -> Result<(), VmManagerError>;
            fn delete_vm(&self);
        }
    }

    mock! {
        pub RealmClient {}

        #[async_trait]
        impl RealmClient for RealmClient {
            async fn acknowledge_client_connection(&mut self, cid: u32) -> Result<(), RealmClientError>;
        }
    }

    mock! {
        pub ApplicationFabric {}
        impl ApplicationCreator for ApplicationFabric {
            fn create_application(&self, config: ApplicationConfig) -> Box<dyn Application + Send + Sync>;
        }
    }

    mock! {
        pub Application {}
        impl Application for Application {
            fn stop(&mut self) -> Result<(), ApplicationError>;
            fn start(&mut self) -> Result<(), ApplicationError>;
            fn update(&mut self, config: ApplicationConfig) -> Result<(), ApplicationError>;
        }
    }
}
