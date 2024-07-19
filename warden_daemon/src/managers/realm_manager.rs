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

#[derive(Debug, Clone, PartialEq, PartialOrd)]
enum State {
    Halted,
    Provisioning,
    Running,
    NeedReboot,
    Destroyed,
}

pub trait VmManager {
    fn setup_network(&mut self, config: &NetworkConfig);
    fn setup_cpu(&mut self, config: &CpuConfig);
    fn setup_kernel(&mut self, config: &KernelConfig);
    fn setup_memory(&mut self, config: &MemoryConfig);
    fn setup_machine(&mut self, name: &String);
    fn launch_vm(&mut self) -> Result<(), VmManagerError>;
    fn stop_vm(&mut self) -> Result<(), VmManagerError>;
    fn delete_vm(&self) -> Result<(), VmManagerError>;
}

#[derive(Debug, Error)]
pub enum VmManagerError {
    #[error("Unable to launch Vm due to: {0}")]
    LaunchFail(#[from] io::Error),
    #[error("To stop realm's vm you need to launch it first!")]
    VmNotLaunched,
    #[error("Unable to stop realm's vm")]
    StopFail,
    #[error("Unable to destroy realm's vm")]
    DestroyFail,
}

#[derive(Debug, Clone, Error, PartialEq, PartialOrd)]
pub enum RealmClientError {
    #[error("Can't connect with the Realm, error: {0}")]
    RealmConnectorError(String),
    #[error("Can't communicate with connected Realm, error: {0}")]
    CommunicationFail(String),
    #[error("Not connected to the realm!")]
    MissingConnection,
}

#[async_trait]
pub trait RealmClient {
    async fn acknowledge_client_connection(&mut self, cid: u32) -> Result<(), RealmClientError>;
    async fn create_application(
        &mut self,
        config: &ApplicationConfig,
    ) -> Result<(), RealmClientError>;
    async fn start_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError>;
    async fn stop_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError>;
}

pub struct RealmManager {
    state: State,
    config: RealmConfig,
    apps_map: Mutex<HashMap<Uuid, Arc<Mutex<Box<dyn Application + Send + Sync>>>>>,
    vm_manager: Box<dyn VmManager + Send + Sync>,
    realm_client: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
    application_fabric: Arc<Box<dyn ApplicationCreator + Send + Sync>>,
}

impl RealmManager {
    pub fn new(
        config: RealmConfig,
        vm_manager: Box<dyn VmManager + Send + Sync>,
        realm_client: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
        application_fabric: Arc<Box<dyn ApplicationCreator + Send + Sync>>,
    ) -> Self {
        RealmManager {
            state: State::Halted,
            apps_map: Mutex::new(HashMap::new()),
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
            .lock()
            .await
            .acknowledge_client_connection(self.config.network.vsock_cid)
            .await
        {
            Ok(_) => {
                self.state = State::Running;
                Ok(())
            }
            Err(err) => {
                self.state = State::Halted;
                Err(RealmError::RealmCantStart(format!("{err}")))
            }
        }
    }

    fn stop(&mut self) -> Result<(), RealmError> {
        if self.state != State::NeedReboot && self.state != State::Running {
            return Err(RealmError::UnsupportedAction(format!(
                "Can't stop realm that is in {:#?} state!",
                self.state
            )));
        }
        self.vm_manager
            .stop_vm()
            .map_err(|err| RealmError::VmStopFail(format!("{}", err)))?;
        self.state = State::Halted;
        Ok(())
    }

    fn destroy(&mut self) -> Result<(), RealmError> {
        if self.state != State::Halted {
            return Err(RealmError::UnsupportedAction(format!(
                "Can't delete realm that isn't halted!"
            )));
        }
        self.vm_manager
            .delete_vm()
            .map_err(|err| RealmError::VmDestroyFail(format!("{}", err)))?;
        self.state = State::Destroyed;
        Ok(())
    }

    async fn reboot(&mut self) -> Result<(), RealmError> {
        self.stop()?;
        self.start().await
    }

    async fn create_application(&mut self, config: ApplicationConfig) -> Result<Uuid, RealmError> {
        if self.state == State::Destroyed {
            return Err(RealmError::UnsupportedAction(String::from(
                "Can't create application!",
            )));
        }
        let uuid = Uuid::new_v4();
        self.realm_client
            .lock()
            .await
            .create_application(&config)
            .await
            .map_err(|err| RealmError::ApplicationCreationFail(format!("{}", err)))?;
        let application = self
            .application_fabric
            .create_application(config, self.realm_client.clone());

        let _ = self
            .apps_map
            .lock()
            .await
            .insert(uuid, Arc::new(Mutex::new(application)));
        Ok(uuid)
    }

    async fn get_application(
        &self,
        uuid: Uuid,
    ) -> Result<Arc<Mutex<Box<dyn Application + Send + Sync>>>, RealmError> {
        if self.state != State::Running && self.state != State::NeedReboot {
            return Err(RealmError::UnsupportedAction(String::from(
                "Can't get application while realm isn't running",
            )));
        }
        match self.apps_map.lock().await.get(&uuid) {
            Some(app_manager) => Ok(app_manager.clone()),
            None => Err(RealmError::ApplicationMissing(uuid)),
        }
    }
    fn signal_reboot(&mut self) {
        self.state = State::NeedReboot;
    }

    fn get_realm_data(&self) -> RealmData {
        RealmData {}
    }
}

impl RealmManager {
    fn setup_vm(&mut self) -> Result<(), RealmError> {
        self.vm_manager.setup_cpu(&self.config.cpu);
        self.vm_manager.setup_kernel(&self.config.kernel);
        self.vm_manager.setup_memory(&self.config.memory);
        self.vm_manager.setup_machine(&self.config.machine);
        self.vm_manager.setup_network(&self.config.network);
        self.vm_manager
            .launch_vm()
            .map_err(|err| RealmError::RealmLaunchFail(err.to_string()))
    }
}

#[cfg(test)]
mod test {
    use std::{io::Error, path::PathBuf, sync::Arc};

    use async_trait::async_trait;
    use mockall::mock;
    use parameterized::parameterized;
    use tokio::sync::Mutex;
    use uuid::Uuid;

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
        let realm_manager = create_realm_manager(create_example_realm_config(), None, None);
        assert_eq!(realm_manager.state, State::Halted);
    }

    #[tokio::test]
    async fn realm_start() {
        let mut realm_manager = create_realm_manager(create_example_realm_config(), None, None);
        assert_eq!(realm_manager.start().await, Ok(()));
        assert_eq!(realm_manager.state, State::Running);
    }

    #[tokio::test]
    #[parameterized(state = {State::Provisioning, State::Running, State::NeedReboot})]
    async fn realm_start_invalid_action(state: State) {
        let mut realm_manager = create_realm_manager(create_example_realm_config(), None, None);
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
        vm_manager_mock.expect_setup_kernel().returning(|_| ());
        vm_manager_mock.expect_setup_machine().returning(|_| ());
        vm_manager_mock.expect_setup_memory().returning(|_| ());
        vm_manager_mock.expect_setup_network().returning(|_| ());
        vm_manager_mock
            .expect_launch_vm()
            .returning(|| Err(VmManagerError::LaunchFail(Error::other(""))));
        let mut realm_manager =
            create_realm_manager(create_example_realm_config(), Some(vm_manager_mock), None);
        assert_eq!(
            realm_manager.start().await,
            Err(RealmError::RealmLaunchFail(String::from(
                "Unable to launch Vm due to: "
            )))
        );
        assert_eq!(realm_manager.state, State::Halted);
    }

    #[tokio::test]
    async fn realm_start_launching_acknowledgment_error() {
        let mut client_mock = MockRealmClient::new();
        client_mock
            .expect_acknowledge_client_connection()
            .return_once(move |_| Err(RealmClientError::RealmConnectorError(String::new())));
        let mut realm_manager =
            create_realm_manager(create_example_realm_config(), None, Some(client_mock));
        assert_eq!(
            realm_manager.start().await,
            Err(RealmError::RealmCantStart(format!(
                "{}",
                RealmClientError::RealmConnectorError(String::new())
            )))
        );
        assert_eq!(realm_manager.state, State::Halted);
    }

    #[tokio::test]
    async fn create_application() {
        let mut realm_manager = create_realm_manager(create_example_realm_config(), None, None);
        let uuid = realm_manager
            .create_application(create_example_application_config())
            .await;
        assert!(uuid.is_ok());
        assert!(realm_manager
            .apps_map
            .lock()
            .await
            .contains_key(&uuid.unwrap()));
    }

    #[tokio::test]
    #[parameterized(state = {State::Running, State::NeedReboot})]
    async fn stop_realm(state: State) {
        let mut realm_manager = create_realm_manager(create_example_realm_config(), None, None);
        realm_manager.state = state;
        assert_eq!(realm_manager.stop(), Ok(()));
        assert_eq!(realm_manager.state, State::Halted);
    }

    #[tokio::test]
    #[parameterized(state = {State::Halted, State::Provisioning})]
    async fn stop_realm_invalid_state(state: State) {
        let mut realm_manager = create_realm_manager(create_example_realm_config(), None, None);
        realm_manager.state = state.clone();
        assert_eq!(
            realm_manager.stop(),
            Err(RealmError::UnsupportedAction(format!(
                "Can't stop realm that is in {:#?} state!",
                state
            )))
        );
    }

    #[tokio::test]
    #[parameterized(state = {State::Running, State::NeedReboot})]
    async fn stop_realm_vm_error(state: State) {
        let mut vm_manager = MockVmManager::new();
        vm_manager
            .expect_stop_vm()
            .return_once(|| Err(VmManagerError::StopFail));
        let mut realm_manager =
            create_realm_manager(create_example_realm_config(), Some(vm_manager), None);
        realm_manager.state = state.clone();
        assert_eq!(
            realm_manager.stop(),
            Err(RealmError::VmStopFail(format!(
                "{}",
                VmManagerError::StopFail
            )))
        );
        assert_eq!(realm_manager.state, state);
    }

    #[test]
    fn destroy_realm() {
        let mut realm_manager = create_realm_manager(create_example_realm_config(), None, None);
        realm_manager.state = State::Halted;
        assert!(realm_manager.destroy().is_ok());
        assert_eq!(realm_manager.state, State::Destroyed);
    }

    #[tokio::test]
    #[parameterized(state = {State::Running, State::NeedReboot, State::Provisioning})]
    async fn destroy_realm_invalid_state(state: State) {
        let mut realm_manager = create_realm_manager(create_example_realm_config(), None, None);
        realm_manager.state = state.clone();
        assert_eq!(
            realm_manager.destroy(),
            Err(RealmError::UnsupportedAction(String::from(
                "Can't delete realm that isn't halted!"
            )))
        );
        assert_eq!(realm_manager.state, state);
    }

    #[test]
    fn destroy_realm_vm_error() {
        const STATE: State = State::Halted;
        let mut vm_manager = MockVmManager::new();
        vm_manager
            .expect_delete_vm()
            .returning(|| Err(VmManagerError::DestroyFail));
        let mut realm_manager =
            create_realm_manager(create_example_realm_config(), Some(vm_manager), None);
        realm_manager.state = STATE;
        assert_eq!(
            realm_manager.destroy(),
            Err(RealmError::VmDestroyFail(format!(
                "{}",
                VmManagerError::DestroyFail
            )))
        );
        assert_eq!(realm_manager.state, STATE);
    }

    #[test]
    fn signal_reboot() {
        let mut realm_manager = create_realm_manager(create_example_realm_config(), None, None);
        realm_manager.signal_reboot();
        assert_eq!(realm_manager.state, State::NeedReboot);
    }

    #[tokio::test]
    async fn destroyed_state() {
        let mut realm_manager = create_realm_manager(create_example_realm_config(), None, None);
        realm_manager.state = State::Destroyed;
        assert!(realm_manager.start().await.is_err());
        assert!(realm_manager.reboot().await.is_err());
        assert!(realm_manager.stop().is_err());
        assert!(realm_manager.destroy().is_err());
        assert!(realm_manager
            .create_application(create_example_application_config())
            .await
            .is_err());
    }

    #[tokio::test]
    #[parameterized(state = {State::Halted, State::Provisioning})]
    async fn get_application_invalid_command(state: State) {
        let mut realm_manager = create_realm_manager(create_example_realm_config(), None, None);
        realm_manager.state = state;
        let uuid = Uuid::new_v4();
        assert_eq!(
            realm_manager.get_application(uuid).await.err().unwrap(),
            RealmError::UnsupportedAction(String::from(
                "Can't get application while realm isn't running"
            ))
        );
    }

    #[tokio::test]
    #[parameterized(state = {State::Running, State::NeedReboot})]
    async fn get_application_missing_applciation(state: State) {
        let mut realm_manager = create_realm_manager(create_example_realm_config(), None, None);
        realm_manager.state = state;
        let uuid = Uuid::new_v4();
        assert_eq!(
            realm_manager.get_application(uuid).await.err().unwrap(),
            RealmError::ApplicationMissing(uuid)
        );
    }

    #[tokio::test]
    #[parameterized(state = {State::Running, State::NeedReboot})]
    async fn get_application(state: State) {
        let mut realm_manager = create_realm_manager(create_example_realm_config(), None, None);
        realm_manager.state = state;
        let uuid = realm_manager
            .create_application(create_example_application_config())
            .await
            .unwrap();
        assert!(realm_manager.get_application(uuid).await.is_ok());
    }

    fn create_realm_manager(
        config: RealmConfig,
        vm_manager: Option<MockVmManager>,
        realm_client: Option<MockRealmClient>,
    ) -> RealmManager {
        let mut vm_manager = vm_manager.unwrap_or(MockVmManager::new());
        vm_manager.expect_setup_cpu().returning(|_| ());
        vm_manager.expect_setup_kernel().returning(|_| ());
        vm_manager.expect_setup_machine().returning(|_| ());
        vm_manager.expect_setup_memory().returning(|_| ());
        vm_manager.expect_setup_network().returning(|_| ());
        vm_manager.expect_launch_vm().returning(|| Ok(()));
        vm_manager.expect_stop_vm().returning(|| Ok(()));
        vm_manager.expect_delete_vm().returning(|| Ok(()));
        let mut realm_client = realm_client.unwrap_or(MockRealmClient::new());
        realm_client
            .expect_acknowledge_client_connection()
            .returning(|_| Ok(()));
        realm_client
            .expect_create_application()
            .returning(|_| Ok(()));
        realm_client
            .expect_start_application()
            .returning(|_| Ok(()));
        realm_client.expect_stop_application().returning(|_| Ok(()));

        let app_mock = MockApplication::new();
        let mut creator_mock = MockApplicationFabric::new();
        creator_mock
            .expect_create_application()
            .return_once(move |_, _| Box::new(app_mock));
        RealmManager::new(
            config,
            Box::new(vm_manager),
            Arc::new(Mutex::new(Box::new(realm_client))),
            Arc::new(Box::new(creator_mock)),
        )
    }

    fn create_example_realm_config() -> RealmConfig {
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

    fn create_example_application_config() -> ApplicationConfig {
        ApplicationConfig {
            uuid: Uuid::new_v4(),
        }
    }

    mock! {
        pub VmManager {}

        impl VmManager for VmManager {
            fn setup_network(&mut self, config: &NetworkConfig);
            fn setup_cpu(&mut self, config: &CpuConfig);
            fn setup_kernel(&mut self, config: &KernelConfig);
            fn setup_memory(&mut self, config: &MemoryConfig);
            fn setup_machine(&mut self, name: &String);
            fn launch_vm(&mut self) -> Result<(), VmManagerError>;
            fn stop_vm(&mut self) -> Result<(), VmManagerError>;
            fn delete_vm(&self) -> Result<(), VmManagerError>;
        }
    }

    mock! {
        pub RealmClient {}

        #[async_trait]
        impl RealmClient for RealmClient {
            async fn acknowledge_client_connection(&mut self, cid: u32) -> Result<(), RealmClientError>;
            async fn create_application(&mut self, config:& ApplicationConfig) -> Result<(), RealmClientError>;
            async fn start_application(&mut self, application_uuid:& Uuid) -> Result<(), RealmClientError>;
            async fn stop_application(&mut self, application_uuid:& Uuid)-> Result<(), RealmClientError>;
        }
    }

    mock! {
        pub ApplicationFabric {}
        impl ApplicationCreator for ApplicationFabric {
            fn create_application(&self, config: ApplicationConfig, realm_client: Arc<tokio::sync::Mutex<Box<dyn RealmClient + Send + Sync>>>) -> Box<dyn Application + Send + Sync>;
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
}
