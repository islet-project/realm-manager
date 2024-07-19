use std::sync::Arc;

use tokio::sync::Mutex;

use crate::managers::{
    application::{Application, ApplicationConfig, ApplicationCreator},
    application_manager::ApplicationManager,
    realm_manager::RealmClient,
};

pub struct ApplicationFabric {}

impl ApplicationFabric {
    pub fn new() -> Self {
        ApplicationFabric {}
    }
}

impl ApplicationCreator for ApplicationFabric {
    fn create_application(
        &self,
        config: ApplicationConfig,
        realm_client: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
    ) -> Box<dyn Application + Send + Sync> {
        Box::new(ApplicationManager::new(config, realm_client))
    }
}
