use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{io, sync::Arc};
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tokio::{select, sync::oneshot::Receiver};
use uuid::Uuid;

use crate::managers::realm_client::{RealmClient, RealmClientError, RealmProvisioningConfig};

#[derive(Debug, Error)]
pub enum RealmSenderError {
    #[error("Failed to send command: {0}!")]
    SendFail(#[from] io::Error),
}

#[derive(Debug, Deserialize, Serialize)]
pub enum RealmCommand {
    ProvisioningConfig(RealmProvisioningConfig),
    StartApplication(Uuid),
    StopApplication(Uuid),
}

#[async_trait]
pub trait RealmConnector {
    async fn acquire_realm_sender(
        &mut self,
        cid: u32,
    ) -> Receiver<Box<dyn RealmSender + Send + Sync>>;
}

#[async_trait]
pub trait RealmSender {
    async fn send(&mut self, data: RealmCommand) -> Result<(), RealmSenderError>;
}

pub struct RealmClientHandler {
    realm_connector: Arc<Mutex<dyn RealmConnector + Send + Sync>>,
    realm_sender: Option<Box<dyn RealmSender + Send + Sync>>,
}

impl RealmClientHandler {
    pub fn new(realm_connector: Arc<Mutex<dyn RealmConnector + Send + Sync>>) -> Self {
        RealmClientHandler {
            realm_connector,
            realm_sender: None,
        }
    }
}

#[async_trait]
impl RealmClient for RealmClientHandler {
    async fn send_realm_provisioning_config(
        &mut self,
        realm_provisioning_config: RealmProvisioningConfig,
        cid: u32,
    ) -> Result<(), RealmClientError> {
        const WAITING_TIME: Duration = Duration::from_secs(10);
        let realm_sender_receiver = self
            .realm_connector
            .lock()
            .await
            .acquire_realm_sender(cid)
            .await;

        select! {
            realm_sender = realm_sender_receiver => {
                let realm_sender = realm_sender.map_err(|err| RealmClientError::RealmConnectorError(err.to_string()))?;
                let sender = self.realm_sender.insert(realm_sender);
                sender
                    .send(RealmCommand::ProvisioningConfig(realm_provisioning_config))
                    .await
                    .map_err(|err| RealmClientError::CommunicationFail(err.to_string()))
            }
            _ = sleep(WAITING_TIME) => {
                Err(RealmClientError::RealmConnectorError(String::from("Timeout on listening for realm connection!")))
            }
        }
    }
    async fn start_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError> {
        let realm_sender = self
            .realm_sender
            .as_mut()
            .ok_or(RealmClientError::MissingConnection)?;
        realm_sender
            .send(RealmCommand::StartApplication(*application_uuid))
            .await
            .map_err(|err| RealmClientError::CommunicationFail(err.to_string()))
    }
    async fn stop_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError> {
        let realm_sender = self
            .realm_sender
            .as_mut()
            .ok_or(RealmClientError::MissingConnection)?;
        realm_sender
            .send(RealmCommand::StopApplication(*application_uuid))
            .await
            .map_err(|err| RealmClientError::CommunicationFail(err.to_string()))
    }
}

#[cfg(test)]
mod test {
    use std::{
        io::{self, Error},
        sync::Arc,
    };

    use async_trait::async_trait;
    use mockall::mock;
    use tokio::sync::{
        oneshot::{Receiver, Sender},
        Mutex,
    };
    use uuid::Uuid;

    use crate::managers::realm_client::{RealmClientError, RealmProvisioningConfig};

    use super::{
        RealmClient, RealmClientHandler, RealmCommand, RealmConnector, RealmSender,
        RealmSenderError,
    };

    #[tokio::test]
    async fn acknowledge_client_connection() {
        let mut realm_client_handler = create_realm_client_handler(None, None);
        let cid = 0;
        assert!(realm_client_handler.realm_sender.is_none());
        assert!(realm_client_handler
            .send_realm_provisioning_config(create_realm_provisioning_config(), cid)
            .await
            .is_ok());
        assert!(realm_client_handler.realm_sender.is_some());
    }

    #[tokio::test]
    async fn start_application_sender_error() {
        let mut sender = MockRealmSender::new();
        sender.expect_send().returning(|command| match command {
            RealmCommand::StartApplication(_) => Err(RealmSenderError::SendFail(Error::other(""))),
            _ => Ok(()),
        });
        let mut realm_client_handler = create_realm_client_handler(None, Some(sender));
        realm_client_handler
            .send_realm_provisioning_config(create_realm_provisioning_config(), 0)
            .await
            .unwrap();
        assert_eq!(
            realm_client_handler
                .start_application(&Uuid::new_v4())
                .await,
            Err(RealmClientError::CommunicationFail(format!(
                "Failed to send command: !"
            )))
        );
    }

    #[tokio::test]
    async fn start_application() {
        let mut realm_client_handler = create_realm_client_handler(None, None);
        realm_client_handler
            .send_realm_provisioning_config(create_realm_provisioning_config(), 0)
            .await
            .unwrap();
        assert!(realm_client_handler
            .start_application(&Uuid::new_v4())
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn stop_application_sender_error() {
        let mut sender = MockRealmSender::new();
        sender.expect_send().returning(|command| match command {
            RealmCommand::StopApplication(_) => Err(RealmSenderError::SendFail(Error::other(""))),
            _ => Ok(()),
        });
        let mut realm_client_handler = create_realm_client_handler(None, Some(sender));
        realm_client_handler
            .send_realm_provisioning_config(create_realm_provisioning_config(), 0)
            .await
            .unwrap();
        assert_eq!(
            realm_client_handler.stop_application(&Uuid::new_v4()).await,
            Err(RealmClientError::CommunicationFail(format!(
                "Failed to send command: !"
            )))
        );
    }

    #[tokio::test]
    async fn stop_application() {
        let mut realm_client_handler = create_realm_client_handler(None, None);
        let uuid = Uuid::new_v4();
        assert_eq!(
            realm_client_handler.stop_application(&uuid).await,
            Err(RealmClientError::MissingConnection)
        );
        realm_client_handler
            .send_realm_provisioning_config(create_realm_provisioning_config(), 0)
            .await
            .unwrap();
        assert!(realm_client_handler.stop_application(&uuid).await.is_ok());
    }

    #[tokio::test]
    async fn acknowledge_client_connection_acquire_error() {
        let (_, mut rx): (
            Sender<Box<dyn RealmSender + Send + Sync>>,
            Receiver<Box<dyn RealmSender + Send + Sync>>,
        ) = tokio::sync::oneshot::channel();
        let mut realm_connector = MockRealmConnector::new();
        rx.close();
        realm_connector
            .expect_acquire_realm_sender()
            .return_once(|_| rx);
        let mut realm_client_handler = create_realm_client_handler(Some(realm_connector), None);
        let cid = 0;
        assert!(realm_client_handler
            .send_realm_provisioning_config(create_realm_provisioning_config(), cid)
            .await
            .is_err());
        assert!(realm_client_handler.realm_sender.is_none());
    }

    #[tokio::test]
    async fn acknowledge_client_connection_send_error() {
        let mut realm_sender = MockRealmSender::new();
        realm_sender
            .expect_send()
            .return_once(|_| Err(RealmSenderError::SendFail(io::Error::other(""))));
        let mut realm_client_handler = create_realm_client_handler(None, Some(realm_sender));
        let cid = 0;
        assert_eq!(
            realm_client_handler
                .send_realm_provisioning_config(create_realm_provisioning_config(), cid)
                .await,
            Err(RealmClientError::CommunicationFail(
                RealmSenderError::SendFail(io::Error::other("")).to_string()
            ))
        );
        assert!(realm_client_handler.realm_sender.is_some());
    }

    fn create_realm_client_handler(
        realm_connector: Option<MockRealmConnector>,
        realm_sender: Option<MockRealmSender>,
    ) -> RealmClientHandler {
        let realm_sender = realm_sender.unwrap_or({
            let mut realm_sender = MockRealmSender::new();
            realm_sender.expect_send().returning(|_| Ok(()));
            realm_sender
        });

        let realm_connector = realm_connector.unwrap_or({
            let mut realm_connector = MockRealmConnector::new();
            let (tx, rx): (
                Sender<Box<dyn RealmSender + Send + Sync>>,
                Receiver<Box<dyn RealmSender + Send + Sync>>,
            ) = tokio::sync::oneshot::channel();
            let _ = tx.send(Box::new(realm_sender));
            realm_connector
                .expect_acquire_realm_sender()
                .return_once(move |_| rx);
            realm_connector
        });
        RealmClientHandler::new(Arc::new(Mutex::new(realm_connector)))
    }

    fn create_realm_provisioning_config() -> RealmProvisioningConfig {
        RealmProvisioningConfig {}
    }

    mock! {
        pub RealmConnector {}

        #[async_trait]
        impl RealmConnector for RealmConnector {
            async fn acquire_realm_sender(
                &mut self,
                cid: u32,
            ) -> Receiver<Box<dyn RealmSender + Send + Sync>>;
        }
    }
    mock! {
        pub RealmSender {}

        #[async_trait]
        impl RealmSender for RealmSender {
            async fn send(&mut self, data: RealmCommand) -> Result<(), RealmSenderError>;
        }
    }
}
