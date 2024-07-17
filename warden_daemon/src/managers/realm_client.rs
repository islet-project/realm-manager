use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{io, sync::Arc};
use thiserror::Error;
use tokio::sync::oneshot::Receiver;
use tokio::sync::Mutex;

use super::realm_manager::{RealmClient, RealmClientError};

#[derive(Debug, Error)]
pub enum RealmSenderError {
    #[error("Failed to parse command")]
    CommandParsingFail(RealmCommand),
    #[error("Failed to send command: {0}")]
    SendFail(#[from] io::Error),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RealmCommand {
    ConnectionConfirmation,
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
    async fn acknowledge_client_connection(&mut self, cid: u32) -> Result<(), RealmClientError> {
        let realm_sender_receiver = self
            .realm_connector
            .lock()
            .await
            .acquire_realm_sender(cid)
            .await;
        let realm_sender = realm_sender_receiver
            .await
            .map_err(|_| RealmClientError::NoConnectionWithRealm)?;
        let _ = self.realm_sender.insert(realm_sender);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use async_trait::async_trait;
    use mockall::mock;
    use tokio::sync::{
        oneshot::{Receiver, Sender},
        Mutex,
    };

    use crate::managers::realm_manager::RealmClient;

    use super::{RealmClientHandler, RealmCommand, RealmConnector, RealmSender, RealmSenderError};

    #[tokio::test]
    async fn acknowledge_client_connection() {
        let mut realm_client = create_realm_client(None);
        let cid = 0;
        assert!(realm_client.realm_sender.is_none());
        assert!(realm_client
            .acknowledge_client_connection(cid)
            .await
            .is_ok());
        assert!(realm_client.realm_sender.is_some());
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
        let mut realm_client = create_realm_client(Some(realm_connector));
        let cid = 0;
        assert!(realm_client
            .acknowledge_client_connection(cid)
            .await
            .is_err());
        assert!(realm_client.realm_sender.is_none());
    }

    fn create_realm_client(realm_connector: Option<MockRealmConnector>) -> RealmClientHandler {
        let realm_connector = realm_connector.unwrap_or({
            let mut realm_connector = MockRealmConnector::new();
            let (tx, rx): (
                Sender<Box<dyn RealmSender + Send + Sync>>,
                Receiver<Box<dyn RealmSender + Send + Sync>>,
            ) = tokio::sync::oneshot::channel();
            let _ = tx.send(Box::new(MockRealmSender::new()));
            realm_connector
                .expect_acquire_realm_sender()
                .return_once(move |_| rx);
            realm_connector
        });
        RealmClientHandler::new(Arc::new(Mutex::new(realm_connector)))
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
