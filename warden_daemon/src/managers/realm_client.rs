use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error, PartialEq, PartialOrd)]
pub enum RealmClientError {
    #[error("Can't connect with the Realm: {0}")]
    RealmConnectionFail(String),
    #[error("Can't communicate with connected Realm: {0}")]
    CommunicationFail(String),
    #[error("Received error from Realm daemon: {0}")]
    RealmDaemonError(String),
    #[error("Invalid response from Realm: {0}")]
    InvalidResponse(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RealmProvisioningConfig {}

#[async_trait]
pub trait RealmClient {
    async fn provision_applications(
        &mut self,
        realm_provisioning_config: RealmProvisioningConfig,
        cid: u32,
    ) -> Result<(), RealmClientError>;
    async fn start_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError>;
    async fn stop_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError>;
    async fn kill_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError>;
    async fn shutdown_realm(&mut self) -> Result<(), RealmClientError>;
    async fn reboot_realm(&mut self) -> Result<(), RealmClientError>;
}
