use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ApplicationError {
    #[error("Can't start the application: {0}")]
    ApplicationStartFail(String),
    #[error("Can't stop the application: {0}")]
    ApplicationStopFail(String),
}

#[derive(Debug, Error, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ApplicationClientError {}

#[async_trait]
pub trait Application {
    async fn stop(&mut self) -> Result<(), ApplicationError>;
    async fn start(&mut self) -> Result<(), ApplicationError>;
    fn update(&mut self, config: ApplicationConfig);
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd)]
pub struct ApplicationConfig {}
