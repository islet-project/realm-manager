use crate::utils::repository::Repository;

use super::application::{Application, ApplicationConfig};
use super::realm::{ApplicationCreator, Realm, RealmData, RealmError, State};
use super::realm_client::{RealmClient, RealmProvisioningConfig};
use super::realm_configuration::*;
use super::vm_manager::VmManager;

use async_trait::async_trait;
use log::{debug, error};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use uuid::Uuid;

type AppsMap = HashMap<Uuid, Arc<Mutex<Box<dyn Application + Send + Sync>>>>;

pub struct RealmManager {
    state: State,
    applications: AppsMap,
    config: Box<dyn Repository<Data = RealmConfig> + Send + Sync>,
    vm_manager: Box<dyn VmManager + Send + Sync>,
    realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
    application_fabric: Box<dyn ApplicationCreator + Send + Sync>,
}

impl RealmManager {
    pub fn new(
        config: Box<dyn Repository<Data = RealmConfig> + Send + Sync>,
        applications: AppsMap,
        vm_manager: Box<dyn VmManager + Send + Sync>,
        realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
        application_fabric: Box<dyn ApplicationCreator + Send + Sync>,
    ) -> Self {
        RealmManager {
            state: State::Halted,
            applications,
            config,
            vm_manager,
            realm_client_handler,
            application_fabric,
        }
    }

    async fn create_provisioning_config(&self) -> Result<RealmProvisioningConfig, RealmError> {
        let mut applications_data = vec![];
        for app in self.applications.values() {
            applications_data.push(
                app.lock()
                    .await
                    .get_data()
                    .await
                    .map_err(|err| RealmError::ApplicationOperation(err.to_string()))?,
            );
        }
        Ok(RealmProvisioningConfig { applications_data })
    }

    async fn prepare_applications(&mut self) -> Result<(), RealmError> {
        let mut set = JoinSet::new();
        for application in self.applications.values() {
            let app = application.clone();
            set.spawn(async move {
                app.lock()
                    .await
                    .configure_disk()
                    .await
                    .map_err(|err| RealmError::ApplicationOperation(err.to_string()))
            });
        }
        while let Some(res) = set.join_next().await {
            res.map_err(|err| RealmError::PrepareApplications(err.to_string()))??;
        }
        Ok(())
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

        self.prepare_applications().await?;

        let apps_uuids: Vec<&Uuid> = self.applications.keys().collect();
        self.vm_manager
            .launch_vm(&apps_uuids)
            .map_err(|err| RealmError::RealmLaunchFail(err.to_string()))?;

        self.state = State::Provisioning;

        match self
            .realm_client_handler
            .lock()
            .await
            .provision_applications(
                self.create_provisioning_config().await?,
                self.config.get().network.vsock_cid,
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

    async fn stop(&mut self) -> Result<(), RealmError> {
        if self.state != State::NeedReboot && self.state != State::Running {
            return Err(RealmError::UnsupportedAction(format!(
                "Can't stop realm that is in {:#?} state.",
                self.state
            )));
        }

        self.realm_client_handler
            .lock()
            .await
            .shutdown_realm()
            .await
            .map_err(|err| RealmError::RealmStopFail(err.to_string()))?;
        self.vm_manager
            .stop_vm()
            .map_err(|err| RealmError::VmStopFail(err.to_string()))?;
        self.vm_manager.get_exit_status();
        self.state = State::Halted;
        Ok(())
    }

    async fn reboot(&mut self) -> Result<(), RealmError> {
        if self.state != State::NeedReboot && self.state != State::Running {
            return Err(RealmError::UnsupportedAction(format!(
                "Can't stop realm that is in {:#?} state.",
                self.state
            )));
        }
        self.realm_client_handler
            .lock()
            .await
            .reboot_realm(
                self.create_provisioning_config().await?,
                self.config.get().network.vsock_cid,
            )
            .await
            .map_err(|err| RealmError::RealmStopFail(err.to_string()))?;
        self.state = State::Running;
        Ok(())
    }

    async fn create_application(&mut self, config: ApplicationConfig) -> Result<Uuid, RealmError> {
        if self.state != State::Halted {
            return Err(RealmError::UnsupportedAction(
                "Can't create application when realm is not halted.".to_string(),
            ));
        }
        let uuid = Uuid::new_v4();
        let application = self
            .application_fabric
            .create_application(uuid, config, self.realm_client_handler.clone())
            .await?;

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
                app_manager
                    .lock()
                    .await
                    .update_config(new_config)
                    .await
                    .map_err(|err| RealmError::ApplicationOperation(err.to_string()))?;
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
            applications: self.applications.keys().copied().collect(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::{RealmError, RealmManager};
    use crate::managers::application::ApplicationError;
    use crate::managers::vm_manager::VmManagerError;
    use crate::managers::{realm::Realm, realm_client::RealmClientError, realm_manager::State};
    use crate::utils::test_utilities::{
        create_example_app_config, create_example_application_data, create_example_realm_config,
        MockApplication, MockApplicationFabric, MockRealmClient, MockRealmRepository,
        MockVmManager,
    };
    use parameterized::parameterized;
    use std::collections::HashMap;
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
        vm_manager_mock
            .expect_launch_vm()
            .returning(|_| Err(VmManagerError::LaunchFail(Error::other(""))));
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
            .expect_provision_applications()
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
    async fn create_provisioning_config_no_apps() {
        let realm_manager = create_realm_manager(None, None);
        assert_eq!(
            realm_manager
                .create_provisioning_config()
                .await
                .unwrap()
                .applications_data
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn prepare_applications_no_apps() {
        let mut realm_manager = create_realm_manager(None, None);
        assert!(realm_manager.prepare_applications().await.is_ok());
    }

    #[tokio::test]
    async fn prepare_applications() {
        let mut realm_manager = create_realm_manager(None, None);
        let uuid = realm_manager
            .create_application(create_example_app_config())
            .await
            .unwrap();
        realm_manager.state = State::Running;
        unsafe {
            let mock = std::ptr::addr_of_mut!(*realm_manager
                .get_application(&uuid)
                .unwrap()
                .lock()
                .await
                .as_mut()) as *mut MockApplication;
            mock.as_mut()
                .unwrap()
                .expect_configure_disk()
                .returning(|| Ok(()));
        };
        assert!(matches!(realm_manager.prepare_applications().await, Ok(())));
    }

    #[tokio::test]
    async fn prepare_applications_error() {
        let mut realm_manager = create_realm_manager(None, None);
        let uuid = realm_manager
            .create_application(create_example_app_config())
            .await
            .unwrap();
        realm_manager.state = State::Running;
        unsafe {
            let mock = std::ptr::addr_of_mut!(*realm_manager
                .get_application(&uuid)
                .unwrap()
                .lock()
                .await
                .as_mut()) as *mut MockApplication;
            mock.as_mut()
                .unwrap()
                .expect_configure_disk()
                .returning(|| Err(ApplicationError::DiskOpertaion(String::new())));
        };
        assert!(matches!(
            realm_manager.prepare_applications().await,
            Err(RealmError::ApplicationOperation(_))
        ));
    }

    #[tokio::test]
    async fn create_provisioning_config_app_error() {
        let mut realm_manager = create_realm_manager(None, None);
        let uuid = realm_manager
            .create_application(create_example_app_config())
            .await
            .unwrap();
        realm_manager.state = State::Running;
        unsafe {
            let mock = std::ptr::addr_of_mut!(*realm_manager
                .get_application(&uuid)
                .unwrap()
                .lock()
                .await
                .as_mut()) as *mut MockApplication;
            mock.as_mut()
                .unwrap()
                .expect_get_data()
                .returning(|| Err(ApplicationError::DiskOpertaion(String::new())));
        };

        assert!(matches!(
            realm_manager.create_provisioning_config().await,
            Err(RealmError::ApplicationOperation(_))
        ));
    }

    #[tokio::test]
    async fn create_provisioning_config() {
        let mut realm_manager = create_realm_manager(None, None);
        let uuid = realm_manager
            .create_application(create_example_app_config())
            .await
            .unwrap();
        realm_manager.state = State::Running;
        unsafe {
            let mock = std::ptr::addr_of_mut!(*realm_manager
                .get_application(&uuid)
                .unwrap()
                .lock()
                .await
                .as_mut()) as *mut MockApplication;
            mock.as_mut()
                .unwrap()
                .expect_get_data()
                .returning(|| Ok(create_example_application_data()));
        };

        assert!(realm_manager
            .create_provisioning_config()
            .await
            .unwrap()
            .applications_data
            .get(0)
            .is_some());
    }

    #[tokio::test]
    #[parameterized(state = {State::Running, State::NeedReboot})]
    async fn stop_realm(state: State) {
        let mut realm_manager = create_realm_manager(None, None);
        realm_manager.state = state;
        assert_eq!(realm_manager.stop().await, Ok(()));
        assert_eq!(realm_manager.state, State::Halted);
    }

    #[tokio::test]
    #[parameterized(state = {State::Halted, State::Provisioning})]
    async fn stop_realm_invalid_state(state: State) {
        let mut realm_manager = create_realm_manager(None, None);
        realm_manager.state = state.clone();
        assert_eq!(
            realm_manager.stop().await,
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
            realm_manager.stop().await,
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
        let uuid = realm_manager
            .create_application(create_example_app_config())
            .await;
        assert_eq!(realm_manager.state, State::Halted);
        assert!(uuid.is_ok());
        assert!(realm_manager.applications.contains_key(&uuid.unwrap()));
    }

    #[tokio::test]
    #[parameterized(state = {State::Running, State::Provisioning, State::NeedReboot})]
    async fn create_application_invalid_state(state: State) {
        let mut realm_manager = create_realm_manager(None, None);
        realm_manager.state = state;
        let uuid_res = realm_manager
            .create_application(create_example_app_config())
            .await;
        assert_eq!(
            uuid_res,
            Err(RealmError::UnsupportedAction(
                "Can't create application when realm is not halted.".to_string()
            ))
        );
    }

    #[tokio::test]
    #[parameterized(states = {(State::Running, State::NeedReboot), (State::NeedReboot, State::NeedReboot), (State::Halted, State::Halted)})]
    async fn update_application(states: (State, State)) {
        let (state, expected_state) = states;
        let mut realm_manager = create_realm_manager(None, None);
        let uuid = realm_manager
            .create_application(create_example_app_config())
            .await
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
            .await
            .unwrap();
        realm_manager.state = State::Provisioning;
        let uuid_res = realm_manager
            .update_application(&uuid, create_example_app_config())
            .await;
        assert_eq!(
            uuid_res,
            Err(RealmError::UnsupportedAction(
                "Can't update application when realm is in provisioning phase.".to_string()
            ))
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
            .await
            .unwrap();
        realm_manager.state = state;
        assert!(realm_manager.get_application(&uuid).is_ok());
    }

    fn create_realm_manager(
        vm_manager: Option<MockVmManager>,
        realm_client_handler: Option<MockRealmClient>,
    ) -> RealmManager {
        let mut vm_manager = vm_manager.unwrap_or_default();
        vm_manager.expect_launch_vm().returning(|_| Ok(()));
        vm_manager.expect_stop_vm().returning(|| Ok(()));
        vm_manager.expect_delete_vm().returning(|| Ok(()));
        let mut realm_client_handler = realm_client_handler.unwrap_or_default();
        realm_client_handler
            .expect_provision_applications()
            .returning(|_, _| Ok(()));
        realm_client_handler
            .expect_start_application()
            .returning(|_| Ok(()));
        realm_client_handler
            .expect_stop_application()
            .returning(|_| Ok(()));
        realm_client_handler
            .expect_shutdown_realm()
            .returning(|| Ok(()));
        realm_client_handler
            .expect_reboot_realm()
            .returning(|_, _| Ok(()));
        realm_client_handler
            .expect_kill_application()
            .returning(|_| Ok(()));
        vm_manager.expect_get_exit_status().returning(|| None);

        let mut app_mock = MockApplication::new();
        app_mock.expect_update_config().returning(|_| Ok(()));

        let mut creator_mock = MockApplicationFabric::new();
        creator_mock
            .expect_create_application()
            .return_once(move |_, _, _| Ok(Box::new(app_mock)));

        let mut repository = MockRealmRepository::new();
        repository
            .expect_get()
            .return_const(create_example_realm_config());
        repository.expect_save().returning(|| Ok(()));
        RealmManager::new(
            Box::new(repository),
            HashMap::new(),
            Box::new(vm_manager),
            Arc::new(Mutex::new(Box::new(realm_client_handler))),
            Box::new(creator_mock),
        )
    }
}
