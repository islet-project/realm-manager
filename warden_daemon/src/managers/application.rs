use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::realm_client::RealmClient;

#[derive(Debug, Error, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ApplicationError {
    #[error("Can't start the application due to: {0}")]
    ApplicationStartFail(String),
    #[error("Can't stop the application due to: {0}")]
    ApplicationStopError(String),
}

#[derive(Debug, Error, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ApplicationClientError {}

pub trait ApplicationCreator {
    fn create_application(
        &self,
        uuid: Uuid,
        config: ApplicationConfig,
        realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
    ) -> Box<dyn Application + Send + Sync>;
}

#[async_trait]
pub trait Application {
    async fn stop(&mut self) -> Result<(), ApplicationError>;
    async fn start(&mut self) -> Result<(), ApplicationError>;
    fn update(&mut self, config: ApplicationConfig);
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApplicationConfig {}
