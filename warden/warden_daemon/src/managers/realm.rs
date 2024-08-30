use super::{
    application::{Application, ApplicationConfig},
    realm_client::RealmClient,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{net::IpAddr, sync::Arc};
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Error, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum RealmError {
    #[error("No application with uuid: {0} inside this realm.")]
    ApplicationMissing(Uuid),
    #[error("Error occured while starting realm: {0}")]
    RealmStartFail(String),
    #[error("Error occured while starting realm: {0}")]
    RealmStopFail(String),
    #[error("Error occured while acquiring realm's ips: {0}")]
    RealmAcuireIpsFail(String),
    #[error("Unsupported action: {0}")]
    UnsupportedAction(String),
    #[error("Can't launch the Realm: {0}")]
    RealmLaunchFail(String),
    #[error("Realm's vm can't be stopped: {0}")]
    VmStopFail(String),
    #[error("Realm's vm can't be destroyed: {0}")]
    VmDestroyFail(String),
    #[error("Can't perform action on Application: {0}")]
    ApplicationOperation(String),
    #[error("Can't create application: {0}")]
    ApplicationCreationFail(String),
    #[error("Failed to prepare applications to start: {0}")]
    PrepareApplications(String),
}

#[async_trait]
pub trait Realm {
    async fn create_application(&mut self, config: ApplicationConfig) -> Result<Uuid, RealmError>;
    fn get_application(
        &self,
        uuid: &Uuid,
    ) -> Result<Arc<Mutex<Box<dyn Application + Send + Sync>>>, RealmError>;
    async fn get_realm_data(&self) -> Result<RealmData, RealmError>;
    async fn reboot(&mut self) -> Result<(), RealmError>;
    async fn start(&mut self) -> Result<(), RealmError>;
    async fn stop(&mut self) -> Result<(), RealmError>;
    async fn update_application(
        &mut self,
        uuid: &Uuid,
        new_config: ApplicationConfig,
    ) -> Result<(), RealmError>;
}

#[async_trait]
pub trait ApplicationCreator {
    async fn create_application(
        &self,
        uuid: Uuid,
        config: ApplicationConfig,
        realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
    ) -> Result<Box<dyn Application + Send + Sync>, RealmError>;
    async fn load_application(
        &self,
        realm_id: &Uuid,
        realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
    ) -> Result<Box<dyn Application + Send + Sync>, RealmError>;
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum State {
    Halted,
    Provisioning,
    Running,
    NeedReboot,
}

#[derive(Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct RealmData {
    pub state: State,
    pub applications: Vec<Uuid>,
    pub ips: Vec<IpAddr>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct RealmDescription {
    pub uuid: Uuid,
    pub realm_data: RealmData,
}
