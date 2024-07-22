use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error, PartialEq, PartialOrd)]
pub enum RealmClientError {
    #[error("Can't connect with the Realm, error: {0}")]
    RealmConnectorError(String),
    #[error("Can't communicate with connected Realm, error: {0}")]
    CommunicationFail(String),
    #[error("Not connected to the realm!")]
    MissingConnection,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RealmProvisioningConfig {}

#[async_trait]
pub trait RealmClient {
    async fn send_realm_provisioning_config(
        &mut self,
        realm_provisioning_config: RealmProvisioningConfig,
        cid: u32,
    ) -> Result<(), RealmClientError>;
    async fn start_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError>;
    async fn stop_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError>;
}
