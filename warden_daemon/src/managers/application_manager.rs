use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::{
    application::{Application, ApplicationConfig, ApplicationError},
    realm_client::RealmClient,
};

pub struct ApplicationManager {
    config: ApplicationConfig,
    realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
    new_config: Option<ApplicationConfig>,
}

impl ApplicationManager {
    pub fn new(
        config: ApplicationConfig,
        realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
    ) -> Self {
        ApplicationManager {
            config,
            realm_client_handler,
            new_config: None,
        }
    }
}

#[async_trait]
impl Application for ApplicationManager {
    async fn stop(&mut self) -> Result<(), ApplicationError> {
        self.realm_client_handler
            .lock()
            .await
            .stop_application(&self.config.uuid)
            .await
            .map_err(|err| ApplicationError::ApplicationStopError(format!("{}", err)))?;
        Ok(())
    }
    async fn start(&mut self) -> Result<(), ApplicationError> {
        self.realm_client_handler
            .lock()
            .await
            .start_application(&self.config.uuid)
            .await
            .map_err(|err| ApplicationError::ApplicationStartFail(format!("{}", err)))?;
        Ok(())
    }
    fn update(&mut self, config: ApplicationConfig) {
        self.new_config = Some(config);
    }
}
