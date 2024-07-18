use std::sync::Arc;

use super::{
    application::{Application, ApplicationConfig},
    realm_configuration::RealmConfig,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Error, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum RealmError {
    #[error("Error occured while starting realm. Error information: {0}")]
    RealmCantStart(String),
    #[error("Unsupported action: {0}")]
    UnsupportedAction(String),
    #[error("Can't launch the Realm, error: {0}")]
    RealmLaunchFail(String),
    #[error("Realm's vm can't be stopped because {0}")]
    VmStopFail(String),
    #[error("Realm's vm can't be destroyed because {0}")]
    VmDestroyFail(String),
}

#[async_trait]
pub trait Realm {
    async fn start(&mut self) -> Result<(), RealmError>;
    async fn reboot(&mut self) -> Result<(), RealmError>;
    async fn get_application(
        &self,
        uuid: &Uuid,
    ) -> Result<Arc<Mutex<Box<dyn Application + Send + Sync>>>, RealmError>;
    fn stop(&mut self) -> Result<(), RealmError>;
    fn create_application(&mut self, config: ApplicationConfig) -> Result<Uuid, RealmError>;
    fn get_realm_data(&self) -> RealmData;
    fn destroy(&mut self) -> Result<(), RealmError>;
}

pub trait RealmCreator {
    fn create_realm(&self, config: RealmConfig) -> Box<dyn Realm + Send + Sync>;
}

#[derive(Debug, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct RealmDescription {
    pub uuid: Uuid,
    pub realm_data: RealmData,
}

#[derive(Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct RealmData {}
