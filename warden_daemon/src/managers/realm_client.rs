use async_trait::async_trait;
use uuid::Uuid;
use thiserror::Error;
use super::application::ApplicationConfig;


#[derive(Debug, Clone, Error, PartialEq, PartialOrd)]
pub enum RealmClientError {
    #[error("Can't connect with the Realm, error: {0}")]
    RealmConnectorError(String),
    #[error("Can't communicate with connected Realm, error: {0}")]
    CommunicationFail(String),
    #[error("Not connected to the realm!")]
    MissingConnection,
}

#[async_trait]
pub trait RealmClient {
    async fn acknowledge_client_connection(&mut self, cid: u32) -> Result<(), RealmClientError>;
    async fn create_application(
        &mut self,
        config: &ApplicationConfig,
    ) -> Result<(), RealmClientError>;
    async fn start_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError>;
    async fn stop_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError>;
}