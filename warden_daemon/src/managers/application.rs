use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ApplicationError {
    #[error("Can't start the application: {0}")]
    ApplicationStartFail(String),
    #[error("Can't stop the application: {0}")]
    ApplicationStopFail(String),
    #[error("Storage failure: {0}")]
    StorageOperationFail(String),
}

#[derive(Debug, Error, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ApplicationClientError {}

#[async_trait]
pub trait Application {
    async fn stop(&mut self) -> Result<(), ApplicationError>;
    async fn start(&mut self) -> Result<(), ApplicationError>;
    fn update(&mut self, config: ApplicationConfig);
}

#[async_trait]
pub trait ApplicationConfigRepository {
    async fn save_realm_config(&mut self) -> Result<(), ApplicationError>;
    fn get_application_config(&mut self) -> &mut ApplicationConfig;
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd)]
pub struct ApplicationConfig {}
