use crate::managers::realm_client::{RealmClient, RealmClientError, RealmProvisioningConfig};
use async_trait::async_trait;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tokio::{select, sync::oneshot::Receiver};
use utils::serde::json_framed::JsonFramedError;
use uuid::Uuid;
use warden_realm::{Request, Response};

#[derive(Debug, Error)]
pub enum RealmSenderError {
    #[error("Failed to communicate with Realm daemon: {0}")]
    CommunicationFail(#[from] JsonFramedError),
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
    async fn send(&mut self, request: Request) -> Result<Response, RealmSenderError>;
}

pub struct RealmClientHandler {
    connector: Arc<Mutex<dyn RealmConnector + Send + Sync>>,
    sender: Option<Box<dyn RealmSender + Send + Sync>>,
}

impl RealmClientHandler {
    pub fn new(realm_connector: Arc<Mutex<dyn RealmConnector + Send + Sync>>) -> Self {
        RealmClientHandler {
            connector: realm_connector,
            sender: None,
        }
    }

    async fn send_command(&mut self, request: Request) -> Result<Response, RealmClientError> {
        let realm_sender = self
            .sender
            .as_mut()
            .ok_or(RealmClientError::RealmConnectionFail(String::from(
                "Realm isn't connected with Warden daemon.",
            )))?;
        realm_sender
            .send(request)
            .await
            .map_err(|err| RealmClientError::CommunicationFail(err.to_string()))
    }

    fn handle_invalid_response(response: Response) -> RealmClientError {
        match response {
            Response::Error(protocol_error) => {
                RealmClientError::RealmDaemonError(format!("{:#?}", protocol_error))
            }
            invalid_response => {
                RealmClientError::InvalidResponse(format!("{:#?}", invalid_response))
            }
        }
    }

    fn handle_success_command(response: Response) -> Result<(), RealmClientError> {
        match response {
            Response::Success() => Ok(()),
            other_response => Err(RealmClientHandler::handle_invalid_response(other_response)),
        }
    }
}

#[async_trait]
impl RealmClient for RealmClientHandler {
    async fn provision_applications(
        &mut self,
        realm_provisioning_config: RealmProvisioningConfig,
        cid: u32,
    ) -> Result<(), RealmClientError> {
        const WAITING_TIME: Duration = Duration::from_secs(10);
        let realm_sender_receiver = self.connector.lock().await.acquire_realm_sender(cid).await;

        select! {
            realm_sender = realm_sender_receiver => {
                let _ = self.sender.insert(realm_sender.map_err(|err| RealmClientError::RealmConnectionFail(err.to_string()))?);
                match self.send_command(Request::ProvisionInfo(realm_provisioning_config.into())).await? {
                    Response::ProvisioningFinished() => Ok(()),
                    other_response => Err(RealmClientHandler::handle_invalid_response(other_response)),
                }
            }
            _ = sleep(WAITING_TIME) => {
                Err(RealmClientError::RealmConnectionFail(String::from("Timeout on listening for realm connection.")))
            }
        }
    }
    async fn start_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError> {
        RealmClientHandler::handle_success_command(
            self.send_command(Request::StartApp(*application_uuid))
                .await
                .map_err(|err| RealmClientError::CommunicationFail(err.to_string()))?,
        )
    }
    async fn stop_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError> {
        RealmClientHandler::handle_success_command(
            self.send_command(Request::StopApp(*application_uuid))
                .await
                .map_err(|err| RealmClientError::CommunicationFail(err.to_string()))?,
        )
    }
    async fn kill_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError> {
        RealmClientHandler::handle_success_command(
            self.send_command(Request::KillApp(*application_uuid))
                .await
                .map_err(|err| RealmClientError::CommunicationFail(err.to_string()))?,
        )
    }
    async fn shutdown_realm(&mut self) -> Result<(), RealmClientError> {
        RealmClientHandler::handle_success_command(
            self.send_command(Request::Shutdown())
                .await
                .map_err(|err| RealmClientError::CommunicationFail(err.to_string()))?,
        )
    }
    async fn reboot_realm(&mut self) -> Result<(), RealmClientError> {
        RealmClientHandler::handle_success_command(
            self.send_command(Request::Reboot())
                .await
                .map_err(|err| RealmClientError::CommunicationFail(err.to_string()))?,
        )
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;
    use tokio::sync::{
        oneshot::{Receiver, Sender},
        Mutex,
    };
    use utils::serde::json_framed::JsonFramedError;
    use uuid::Uuid;
    use warden_realm::{ProtocolError, Request, Response};

    use super::{RealmClient, RealmClientError, RealmClientHandler, RealmSender, RealmSenderError};
    use crate::utils::test_utilities::{
        create_realm_provisioning_config, MockRealmConnector, MockRealmSender,
    };

    #[tokio::test]
    async fn send_command() {
        let mut realm_sender_mock = MockRealmSender::new();
        realm_sender_mock
            .expect_send()
            .returning(|_| Ok(Response::Success()));

        let mut realm_client_handler = create_realm_client_handler(None, None);
        realm_client_handler.sender = Some(Box::new(realm_sender_mock));
        assert!(realm_client_handler
            .send_command(Request::Reboot())
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn send_command_send_issue() {
        const ERR: RealmSenderError =
            RealmSenderError::CommunicationFail(JsonFramedError::StreamIsClosed());
        let mut realm_sender_mock = MockRealmSender::new();
        realm_sender_mock.expect_send().returning(|_| Err(ERR));

        let mut realm_client_handler = create_realm_client_handler(None, None);
        realm_client_handler.sender = Some(Box::new(realm_sender_mock));
        assert!(
            match realm_client_handler.send_command(Request::Reboot()).await {
                Err(RealmClientError::CommunicationFail(_)) => true,
                _ => false,
            }
        );
    }

    #[tokio::test]
    async fn send_command_connection_issue() {
        let mut realm_client_handler = create_realm_client_handler(None, None);
        assert!(
            match realm_client_handler.send_command(Request::Reboot()).await {
                Err(RealmClientError::RealmConnectionFail(_)) => true,
                _ => false,
            }
        );
    }

    #[test]
    fn handle_success_response() {
        const RESP: Response = Response::Success();
        assert!(match RealmClientHandler::handle_success_command(RESP) {
            Ok(()) => true,
            _ => false,
        })
    }

    #[test]
    fn handle_not_success_response() {
        const RESP: Response = Response::ProvisioningFinished();
        assert!(match RealmClientHandler::handle_success_command(RESP) {
            Ok(()) => false,
            _ => true,
        })
    }

    #[test]
    fn handle_invalid_response() {
        const RESP: Response = Response::Error(ProtocolError::ApplicationsAlreadyProvisioned());
        assert!(match RealmClientHandler::handle_invalid_response(RESP) {
            RealmClientError::RealmDaemonError(_) => true,
            _ => false,
        })
    }

    #[test]
    fn handle_invalid_other_response() {
        const RESP: Response = Response::ProvisioningFinished();
        assert!(match RealmClientHandler::handle_invalid_response(RESP) {
            RealmClientError::InvalidResponse(_) => true,
            _ => false,
        })
    }

    #[tokio::test]
    async fn acknowledge_client_connection() {
        let mut realm_sender_mock = MockRealmSender::new();
        realm_sender_mock
            .expect_send()
            .returning(|_| Ok(Response::ProvisioningFinished()));
        let mut realm_client_handler = create_realm_client_handler(None, Some(realm_sender_mock));
        let cid = 0;
        assert!(realm_client_handler.sender.is_none());
        assert!(realm_client_handler
            .provision_applications(create_realm_provisioning_config(), cid)
            .await
            .is_ok());
        assert!(realm_client_handler.sender.is_some());
    }

    #[tokio::test]
    async fn start_application() {
        let app_uuid = Uuid::new_v4();
        let sender_mock = {
            let mut mock = MockRealmSender::new();
            let uuid_cpy = app_uuid.clone();
            mock.expect_send().return_once(move |req| match req {
                Request::StartApp(uuid) if uuid == uuid_cpy => Ok(Response::Success()),
                _ => Ok(Response::ApplicationNotStarted()),
            });
            mock
        };
        let mut realm_client_handler = create_realm_client_handler(None, None);
        realm_client_handler.sender = Some(Box::new(sender_mock));
        assert!(realm_client_handler
            .start_application(&app_uuid)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn stop_application() {
        let app_uuid = Uuid::new_v4();
        let sender_mock = {
            let mut mock = MockRealmSender::new();
            let uuid_cpy = app_uuid.clone();
            mock.expect_send().return_once(move |req| match req {
                Request::StopApp(uuid) if uuid == uuid_cpy => Ok(Response::Success()),
                _ => Ok(Response::ApplicationNotStarted()),
            });
            mock
        };
        let mut realm_client_handler = create_realm_client_handler(None, None);
        realm_client_handler.sender = Some(Box::new(sender_mock));
        assert!(realm_client_handler
            .stop_application(&app_uuid)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn kill_application() {
        let app_uuid = Uuid::new_v4();
        let sender_mock = {
            let mut mock = MockRealmSender::new();
            let uuid_cpy = app_uuid.clone();
            mock.expect_send().return_once(move |req| match req {
                Request::KillApp(uuid) if uuid == uuid_cpy => Ok(Response::Success()),
                _ => Ok(Response::ApplicationNotStarted()),
            });
            mock
        };
        let mut realm_client_handler = create_realm_client_handler(None, None);
        realm_client_handler.sender = Some(Box::new(sender_mock));
        assert!(realm_client_handler
            .kill_application(&app_uuid)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn shutdown_realm() {
        let sender_mock = {
            let mut mock = MockRealmSender::new();
            mock.expect_send().return_once(move |req| match req {
                Request::Shutdown() => Ok(Response::Success()),
                _ => Ok(Response::ApplicationNotStarted()),
            });
            mock
        };
        let mut realm_client_handler = create_realm_client_handler(None, None);
        realm_client_handler.sender = Some(Box::new(sender_mock));
        assert!(realm_client_handler.shutdown_realm().await.is_ok());
    }

    #[tokio::test]
    async fn reboot_realm() {
        let sender_mock = {
            let mut mock = MockRealmSender::new();
            mock.expect_send().return_once(move |req| match req {
                Request::Reboot() => Ok(Response::Success()),
                _ => Ok(Response::ApplicationNotStarted()),
            });
            mock
        };
        let mut realm_client_handler = create_realm_client_handler(None, None);
        realm_client_handler.sender = Some(Box::new(sender_mock));
        assert!(realm_client_handler.reboot_realm().await.is_ok());
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
            .provision_applications(create_realm_provisioning_config(), cid)
            .await
            .is_err());
        assert!(realm_client_handler.sender.is_none());
    }

    fn create_realm_client_handler(
        realm_connector: Option<MockRealmConnector>,
        realm_sender: Option<MockRealmSender>,
    ) -> RealmClientHandler {
        let realm_sender = realm_sender.unwrap_or({
            let mut realm_sender = MockRealmSender::new();
            realm_sender
                .expect_send()
                .returning(|_| Ok(Response::Success()));
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
}
