use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ApplicationError {}

pub trait ApplicationCreator {
    fn create_application(&self, config: ApplicationConfig) -> Box<dyn Application + Send + Sync>;
}

pub trait Application {
    fn stop(&mut self) -> Result<(), ApplicationError>;
    fn start(&mut self) -> Result<(), ApplicationError>;
    fn update(&mut self, config: ApplicationConfig) -> Result<(), ApplicationError>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApplicationConfig {}
