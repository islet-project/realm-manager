use super::application::{Application, ApplicationConfig, ApplicationCreator};
use super::realm::{Realm, RealmData, RealmError, State};
use super::realm_client::{RealmClient, RealmProvisioningConfig};
use super::realm_configuration::*;

use async_trait::async_trait;
use log::{debug, error};
use std::collections::HashMap;
use std::io;
use std::process::ExitStatus;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

pub trait VmManager {
    fn delete_vm(&mut self) -> Result<(), VmManagerError>;
    fn get_exit_status(&mut self) -> Option<ExitStatus>;
    fn launch_vm(&mut self) -> Result<(), VmManagerError>;
    fn setup_cpu(&mut self, config: &CpuConfig);
    fn setup_kernel(&mut self, config: &KernelConfig);
    fn setup_machine(&mut self, name: &str);
    fn setup_memory(&mut self, config: &MemoryConfig);
    fn setup_network(&mut self, config: &NetworkConfig);
    fn stop_vm(&mut self) -> Result<(), VmManagerError>;
}

#[derive(Debug, Error)]
pub enum VmManagerError {
    #[error("Unable to launch Vm: {0}")]
    LaunchFail(#[from] io::Error),
    #[error("To stop realm's vm you need to launch it first.")]
    VmNotLaunched,
    #[error("Unable to stop realm's vm.")]
    StopFail,
    #[error("Unable to destroy realm's vm: {0}")]
    DestroyFail(String),
}

type AppsMap = HashMap<Uuid, Arc<Mutex<Box<dyn Application + Send + Sync>>>>;

pub struct RealmManager {
    state: State,
    config: RealmConfig,
    applications: AppsMap,
    vm_manager: Box<dyn VmManager + Send + Sync>,
    realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
    application_fabric: Arc<Box<dyn ApplicationCreator + Send + Sync>>,
}

impl RealmManager {
    pub fn new(
        config: RealmConfig,
        vm_manager: Box<dyn VmManager + Send + Sync>,
        realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
        application_fabric: Arc<Box<dyn ApplicationCreator + Send + Sync>>,
    ) -> Self {
        RealmManager {
            state: State::Halted,
            applications: HashMap::new(),
            config,
            vm_manager,
            realm_client_handler,
            application_fabric,
        }
    }

    fn create_provisioning_config(&self) -> RealmProvisioningConfig {
        RealmProvisioningConfig {}
    }

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

impl Drop for RealmManager {
    fn drop(&mut self) {
        debug!("Called destructor for RealmManager.");
        if let Err(error) = self.vm_manager.delete_vm() {
            error!(
                "Error occured while dropping RealmManager: {}",
                RealmError::VmDestroyFail(error.to_string())
            );
        }
    }
}

#[async_trait]
impl Realm for RealmManager {
    async fn start(&mut self) -> Result<(), RealmError> {
        if self.state != State::Halted {
            return Err(RealmError::UnsupportedAction(String::from(
                "Can't start realm that is not halted.",
            )));
        }

        self.setup_vm()?;

        self.state = State::Provisioning;

        match self
            .realm_client_handler
            .lock()
            .await
            .send_realm_provisioning_config(
                self.create_provisioning_config(),
                self.config.network.vsock_cid,
            )
            .await
        {
            Ok(_) => {
                self.state = State::Running;
                Ok(())
            }
            Err(err) => {
                self.state = State::Halted;
                let mut error = err.to_string();
                if let Some(runner_error) = self.vm_manager.get_exit_status() {
                    error = format!("{error}, {}", runner_error);
                }
                Err(RealmError::RealmStartFail(error))
            }
        }
    }

    fn stop(&mut self) -> Result<(), RealmError> {
        if self.state != State::NeedReboot && self.state != State::Running {
            return Err(RealmError::UnsupportedAction(format!(
                "Can't stop realm that is in {:#?} state.",
                self.state
            )));
        }
        self.vm_manager
            .stop_vm()
            .map_err(|err| RealmError::VmStopFail(err.to_string()))?;
        self.state = State::Halted;
        Ok(())
    }

    async fn reboot(&mut self) -> Result<(), RealmError> {
        self.stop()?;
        self.start().await
    }

    fn create_application(&mut self, config: ApplicationConfig) -> Result<Uuid, RealmError> {
        if self.state != State::Halted {
            return Err(RealmError::UnsupportedAction(
                "Can't create application when realm is not halted.".to_string(),
            ));
        }
        let uuid = Uuid::new_v4();
        let application = self.application_fabric.create_application(
            uuid,
            config,
            self.realm_client_handler.clone(),
        );

        let _ = self
            .applications
            .insert(uuid, Arc::new(Mutex::new(application)));
        Ok(uuid)
    }

    async fn update_application(
        &mut self,
        uuid: &Uuid,
        new_config: ApplicationConfig,
    ) -> Result<(), RealmError> {
        if self.state == State::Provisioning {
            return Err(RealmError::UnsupportedAction(
                "Can't update application when realm is in provisioning phase.".to_string(),
            ));
        }
        match self.applications.get(uuid) {
            Some(app_manager) => {
                app_manager.lock().await.update(new_config);
                if self.state == State::Running {
                    self.state = State::NeedReboot;
                }
                Ok(())
            }
            None => Err(RealmError::ApplicationMissing(*uuid)),
        }
    }

    fn get_application(
        &self,
        uuid: &Uuid,
    ) -> Result<Arc<Mutex<Box<dyn Application + Send + Sync>>>, RealmError> {
        if self.state != State::Running && self.state != State::NeedReboot {
            return Err(RealmError::UnsupportedAction(
                "Can't get application while realm isn't running.".to_string(),
            ));
        }
        match self.applications.get(uuid) {
            Some(app_manager) => Ok(app_manager.clone()),
            None => Err(RealmError::ApplicationMissing(*uuid)),
        }
    }

    fn get_realm_data(&self) -> RealmData {
        RealmData {
            state: self.state.clone(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::{RealmError, RealmManager, VmManagerError};
    use crate::managers::{realm::Realm, realm_client::RealmClientError, realm_manager::State};
    use crate::test_utilities::{
        create_example_app_config, create_example_realm_config, MockApplication,
        MockApplicationFabric, MockRealmClient, MockVmManager,
    };
    use parameterized::parameterized;
    use std::{io::Error, sync::Arc};
    use tokio::sync::Mutex;
    use uuid::Uuid;

    #[tokio::test]
    async fn new() {
        let realm_manager = create_realm_manager(None, None);
        assert_eq!(realm_manager.state, State::Halted);
        assert_eq!(realm_manager.applications.len(), 0);
    }

    #[tokio::test]
    async fn realm_start() {
        let mut realm_manager = create_realm_manager(None, None);
        assert_eq!(realm_manager.start().await, Ok(()));
        assert_eq!(realm_manager.state, State::Running);
    }

    #[tokio::test]
    #[parameterized(state = {State::Provisioning, State::Running, State::NeedReboot})]
    async fn realm_start_invalid_action(state: State) {
        let mut realm_manager = create_realm_manager(None, None);
        realm_manager.state = state;
        assert_eq!(
            realm_manager.start().await,
            Err(RealmError::UnsupportedAction(String::from(
                "Can't start realm that is not halted."
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
        let mut realm_manager = create_realm_manager(Some(vm_manager_mock), None);
        assert_eq!(
            realm_manager.start().await,
            Err(RealmError::RealmLaunchFail(String::from(
                "Unable to launch Vm: "
            )))
        );
        assert_eq!(realm_manager.state, State::Halted);
    }

    #[tokio::test]
    async fn realm_start_launching_acknowledgment_error() {
        let mut client_mock = MockRealmClient::new();
        client_mock
            .expect_send_realm_provisioning_config()
            .return_once(|_, _| Err(RealmClientError::RealmConnectionFail(String::new())));
        let mut realm_manager = create_realm_manager(None, Some(client_mock));
        assert_eq!(
            realm_manager.start().await,
            Err(RealmError::RealmStartFail(
                RealmClientError::RealmConnectionFail(String::new()).to_string()
            ))
        );
        assert_eq!(realm_manager.state, State::Halted);
    }

    #[tokio::test]
    #[parameterized(state = {State::Running, State::NeedReboot})]
    async fn stop_realm(state: State) {
        let mut realm_manager = create_realm_manager(None, None);
        realm_manager.state = state;
        assert_eq!(realm_manager.stop(), Ok(()));
        assert_eq!(realm_manager.state, State::Halted);
    }

    #[tokio::test]
    #[parameterized(state = {State::Halted, State::Provisioning})]
    async fn stop_realm_invalid_state(state: State) {
        let mut realm_manager = create_realm_manager(None, None);
        realm_manager.state = state.clone();
        assert_eq!(
            realm_manager.stop(),
            Err(RealmError::UnsupportedAction(format!(
                "Can't stop realm that is in {:#?} state.",
                &state
            )))
        );
        assert_eq!(realm_manager.state, state);
    }

    #[tokio::test]
    #[parameterized(state = {State::Running, State::NeedReboot})]
    async fn stop_realm_vm_error(state: State) {
        let mut vm_manager = MockVmManager::new();
        vm_manager
            .expect_stop_vm()
            .return_once(|| Err(VmManagerError::StopFail));
        let mut realm_manager = create_realm_manager(Some(vm_manager), None);
        realm_manager.state = state.clone();
        assert_eq!(
            realm_manager.stop(),
            Err(RealmError::VmStopFail(VmManagerError::StopFail.to_string()))
        );
        assert_eq!(realm_manager.state, state);
    }

    #[tokio::test]
    #[parameterized(state = {State::Running, State::NeedReboot})]
    async fn reboot(state: State) {
        let mut realm_manager = create_realm_manager(None, None);
        realm_manager.state = state;
        assert_eq!(realm_manager.reboot().await, Ok(()));
        assert_eq!(realm_manager.state, State::Running);
    }

    #[tokio::test]
    async fn create_application() {
        let mut realm_manager = create_realm_manager(None, None);
        let uuid = realm_manager.create_application(create_example_app_config());
        assert_eq!(realm_manager.state, State::Halted);
        assert!(uuid.is_ok());
        assert!(realm_manager.applications.contains_key(&uuid.unwrap()));
    }

    #[tokio::test]
    #[parameterized(state = {State::Running, State::Provisioning, State::NeedReboot})]
    async fn create_application_invalid_state(state: State) {
        let mut realm_manager = create_realm_manager(None, None);
        realm_manager.state = state;
        let uuid_res = realm_manager.create_application(create_example_app_config());
        assert_eq!(
            uuid_res,
            Err(RealmError::UnsupportedAction(format!(
                "Can't create application when realm is not halted.",
            )))
        );
    }

    #[tokio::test]
    #[parameterized(states = {(State::Running, State::NeedReboot), (State::NeedReboot, State::NeedReboot), (State::Halted, State::Halted)})]
    async fn update_application(states: (State, State)) {
        let (state, expected_state) = states;
        let mut realm_manager = create_realm_manager(None, None);
        let uuid = realm_manager
            .create_application(create_example_app_config())
            .unwrap();
        realm_manager.state = state;
        let uuid_res = realm_manager
            .update_application(&uuid, create_example_app_config())
            .await;
        assert_eq!(uuid_res, Ok(()));
        assert_eq!(realm_manager.state, expected_state);
    }

    #[tokio::test]
    async fn update_application_invalid_state() {
        let mut realm_manager = create_realm_manager(None, None);
        let uuid = realm_manager
            .create_application(create_example_app_config())
            .unwrap();
        realm_manager.state = State::Provisioning;
        let uuid_res = realm_manager
            .update_application(&uuid, create_example_app_config())
            .await;
        assert_eq!(
            uuid_res,
            Err(RealmError::UnsupportedAction(format!(
                "Can't update application when realm is in provisioning phase.",
            )))
        );
    }

    #[tokio::test]
    async fn update_missing_application() {
        let mut realm_manager = create_realm_manager(None, None);
        let uuid = Uuid::new_v4();
        realm_manager.state = State::Running;
        let uuid_res = realm_manager
            .update_application(&uuid, create_example_app_config())
            .await;
        assert_eq!(uuid_res, Err(RealmError::ApplicationMissing(uuid)));
    }

    #[tokio::test]
    #[parameterized(state = {State::Halted, State::Provisioning})]
    async fn get_application_invalid_command(state: State) {
        let mut realm_manager = create_realm_manager(None, None);
        realm_manager.state = state;
        let uuid = Uuid::new_v4();
        assert_eq!(
            realm_manager.get_application(&uuid).err().unwrap(),
            RealmError::UnsupportedAction(String::from(
                "Can't get application while realm isn't running."
            ))
        );
    }

    #[tokio::test]
    #[parameterized(state = {State::Running, State::NeedReboot})]
    async fn get_application_missing_applciation(state: State) {
        let mut realm_manager = create_realm_manager(None, None);
        realm_manager.state = state;
        let uuid = Uuid::new_v4();
        assert_eq!(
            realm_manager.get_application(&uuid).err().unwrap(),
            RealmError::ApplicationMissing(uuid)
        );
    }

    #[tokio::test]
    #[parameterized(state = {State::Running, State::NeedReboot})]
    async fn get_application(state: State) {
        let mut realm_manager = create_realm_manager(None, None);
        let uuid = realm_manager
            .create_application(create_example_app_config())
            .unwrap();
        realm_manager.state = state;
        assert!(realm_manager.get_application(&uuid).is_ok());
    }

    fn create_realm_manager(
        vm_manager: Option<MockVmManager>,
        realm_client_handler: Option<MockRealmClient>,
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
        let mut realm_client_handler = realm_client_handler.unwrap_or(MockRealmClient::new());
        realm_client_handler
            .expect_send_realm_provisioning_config()
            .returning(|_, _| Ok(()));
        realm_client_handler
            .expect_start_application()
            .returning(|_| Ok(()));
        realm_client_handler
            .expect_stop_application()
            .returning(|_| Ok(()));
        vm_manager.expect_get_exit_status().returning(|| None);

        let mut app_mock = MockApplication::new();
        app_mock.expect_update().returning(|_| ());

        let mut creator_mock = MockApplicationFabric::new();
        creator_mock
            .expect_create_application()
            .return_once(move |_, _, _| Box::new(app_mock));
        RealmManager::new(
            create_example_realm_config(),
            Box::new(vm_manager),
            Arc::new(Mutex::new(Box::new(realm_client_handler))),
            Arc::new(Box::new(creator_mock)),
        )
    }
}
