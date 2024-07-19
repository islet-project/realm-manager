use std::sync::Arc;

use tokio::sync::Mutex;
use uuid::Uuid;

use crate::managers::{
    application::{Application, ApplicationConfig, ApplicationCreator},
    application_manager::ApplicationManager,
    realm_client::RealmClient,
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
        uuid: Uuid,
        config: ApplicationConfig,
        realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
    ) -> Box<dyn Application + Send + Sync> {
        Box::new(ApplicationManager::new(uuid, config, realm_client_handler))
    }
}
