use crate::managers::realm::RealmNetwork;
use crate::managers::realm_client::{RealmClient, RealmClientError, RealmProvisioningConfig};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tokio::{select, sync::oneshot::Receiver};
use tokio_util::sync::CancellationToken;
use utils::serde::json_framed::JsonFramedError;
use uuid::Uuid;
use warden_realm::{NetAddr, Request, Response};

#[derive(Debug, Error)]
pub enum RealmSenderError {
    #[error("Failed to send request to Realm daemon: {0}")]
    SendFail(#[source] JsonFramedError),
    #[error("Failed to receive message from Realm daemon: {0}")]
    ReceiveFail(#[source] JsonFramedError),
    #[error("Waiting too long for the response.")]
    Timeout,
    #[error("Realm disconnected!")]
    Disconnection,
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
    async fn send(&mut self, request: Request) -> Result<(), RealmSenderError>;
    async fn receive_response(&mut self, timeout: Duration) -> Result<Response, RealmSenderError>;
}

pub struct RealmClientHandler {
    connector: Arc<Mutex<dyn RealmConnector + Send + Sync>>,
    connection_wait_time: Duration,
    response_timeout: Duration,
    cancellation_token: Arc<CancellationToken>,
    sender: Option<Box<dyn RealmSender + Send + Sync>>,
}

impl RealmClientHandler {
    pub fn new(
        realm_connector: Arc<Mutex<dyn RealmConnector + Send + Sync>>,
        realm_connection_wait_time: Duration,
        realm_response_timeout: Duration,
        cancellation_token: Arc<CancellationToken>,
    ) -> Self {
        Self {
            connector: realm_connector,
            connection_wait_time: realm_connection_wait_time,
            response_timeout: realm_response_timeout,
            cancellation_token,
            sender: None,
        }
    }

    fn acquire_realm_sender(
        &mut self,
    ) -> Result<&mut Box<dyn RealmSender + Send + Sync>, RealmClientError> {
        self.sender
            .as_mut()
            .ok_or(RealmClientError::RealmConnectionFail(String::from(
                "Realm isn't connected with Warden daemon.",
            )))
    }

    async fn send_command(&mut self, request: Request) -> Result<(), RealmClientError> {
        let realm_sender = self.acquire_realm_sender()?;
        realm_sender
            .send(request)
            .await
            .map_err(|err| RealmClientError::CommunicationFail(err.to_string()))
    }

    async fn read_response(&mut self, timeout: Duration) -> Result<Response, RealmClientError> {
        let realm_sender = self.acquire_realm_sender()?;
        realm_sender
            .receive_response(timeout)
            .await
            .map_err(|err| match err {
                RealmSenderError::Disconnection => RealmClientError::RealmDisconnection(),
                err => RealmClientError::CommunicationFail(format!("{:#?}", err)),
            })
    }

    async fn handle_shutdown_response(&mut self) -> Result<(), RealmClientError> {
        match self.read_response(self.response_timeout).await {
            Err(RealmClientError::RealmDisconnection()) => {
                self.sender = None;
                Ok(())
            }
            Ok(resp) => Err(Self::handle_invalid_response(resp)),
            err => err.map(|_| ()),
        }
    }

    async fn handle_ip_response(&mut self) -> Result<HashMap<String, NetAddr>, RealmClientError> {
        match self.read_response(self.response_timeout).await? {
            Response::IfAddrs(addrs) => Ok(addrs),
            resp => Err(Self::handle_invalid_response(resp)),
        }
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
            other_response => Err(Self::handle_invalid_response(other_response)),
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
        let realm_sender_receiver = self.connector.lock().await.acquire_realm_sender(cid).await;

        select! {
            realm_sender = realm_sender_receiver => {
                let _ = self.sender.insert(realm_sender.map_err(|err| RealmClientError::RealmConnectionFail(err.to_string()))?);
                self.send_command(Request::ProvisionInfo(realm_provisioning_config.into())).await?;
                Self::handle_success_command(self.read_response(self.connection_wait_time).await?)
            },
            _ = self.cancellation_token.cancelled() => {
                Err(RealmClientError::RealmConnectionFail(String::from("Waiting for realm connection cancelled.")))
            },
            _ = sleep(self.connection_wait_time) => {
                Err(RealmClientError::RealmConnectionFail(String::from("Timeout on listening for realm connection.")))
            }
        }
    }
    async fn start_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError> {
        self.send_command(Request::StartApp(*application_uuid))
            .await?;
        Self::handle_success_command(self.read_response(self.response_timeout).await?)
    }
    async fn stop_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError> {
        self.send_command(Request::StopApp(*application_uuid))
            .await?;
        if Self::handle_success_command(self.read_response(self.response_timeout).await?).is_err() {
            return self.kill_application(application_uuid).await;
        }
        Ok(())
    }
    async fn kill_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError> {
        self.send_command(Request::KillApp(*application_uuid))
            .await?;
        Self::handle_success_command(self.read_response(self.response_timeout).await?)
    }
    async fn shutdown_realm(&mut self) -> Result<(), RealmClientError> {
        self.send_command(Request::Shutdown()).await?;
        self.handle_shutdown_response().await
    }
    async fn reboot_realm(
        &mut self,
        realm_provisioning_config: RealmProvisioningConfig,
        cid: u32,
    ) -> Result<(), RealmClientError> {
        self.send_command(Request::Reboot()).await?;
        self.provision_applications(realm_provisioning_config, cid)
            .await
    }
    async fn read_realm_ifs(&mut self) -> Result<Vec<RealmNetwork>, RealmClientError> {
        self.send_command(Request::GetIfAddrs()).await?;
        Ok(self
            .handle_ip_response()
            .await?
            .into_iter()
            .map(|(if_name, ip)| RealmNetwork {
                ip: ip.address,
                if_name,
            })
            .collect())
    }
}

#[cfg(test)]
mod test {
    use std::{collections::HashMap, net::Ipv4Addr, sync::Arc, time::Duration};
    use tokio::sync::{
        oneshot::{Receiver, Sender},
        Mutex,
    };
    use tokio_util::sync::CancellationToken;
    use utils::serde::json_framed::JsonFramedError;
    use uuid::Uuid;
    use warden_realm::{NetAddr, ProtocolError, Request, Response};

    use super::{RealmClient, RealmClientError, RealmClientHandler, RealmSender, RealmSenderError};
    use crate::utils::test_utilities::{
        create_example_realm_provisioning_config, MockRealmConnector, MockRealmSender,
    };

    const TIMEOUT: Duration = Duration::from_secs(1);

    #[tokio::test]
    async fn send_command() {
        let mut realm_sender_mock = MockRealmSender::new();
        realm_sender_mock.expect_send().returning(|_| Ok(()));

        let mut realm_client_handler = create_realm_client_handler(None, None);
        realm_client_handler.sender = Some(Box::new(realm_sender_mock));
        assert!(realm_client_handler
            .send_command(Request::Reboot())
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn send_command_send_issue() {
        const ERR: RealmSenderError = RealmSenderError::SendFail(JsonFramedError::StreamIsClosed());
        let mut realm_sender_mock = MockRealmSender::new();
        realm_sender_mock.expect_send().returning(|_| Err(ERR));

        let mut realm_client_handler = create_realm_client_handler(None, None);
        realm_client_handler.sender = Some(Box::new(realm_sender_mock));
        assert!(matches!(
            realm_client_handler.send_command(Request::Reboot()).await,
            Err(RealmClientError::CommunicationFail(_))
        ));
    }

    #[tokio::test]
    async fn send_command_connection_issue() {
        let mut realm_client_handler = create_realm_client_handler(None, None);
        assert!(matches!(
            realm_client_handler.send_command(Request::Reboot()).await,
            Err(RealmClientError::RealmConnectionFail(_))
        ));
    }

    #[tokio::test]
    async fn read_response() {
        let mut realm_sender_mock = MockRealmSender::new();
        realm_sender_mock
            .expect_receive_response()
            .returning(|_| Ok(Response::Success()));

        let mut realm_client_handler = create_realm_client_handler(None, None);
        realm_client_handler.sender = Some(Box::new(realm_sender_mock));
        assert!(realm_client_handler.read_response(TIMEOUT).await.is_ok());
    }

    #[tokio::test]
    async fn read_response_disconnection() {
        let mut realm_sender_mock = MockRealmSender::new();
        realm_sender_mock
            .expect_receive_response()
            .returning(|_| Err(RealmSenderError::Disconnection));

        let mut realm_client_handler = create_realm_client_handler(None, None);
        realm_client_handler.sender = Some(Box::new(realm_sender_mock));
        assert!(matches!(
            realm_client_handler.read_response(TIMEOUT).await,
            Err(RealmClientError::RealmDisconnection())
        ));
    }

    #[tokio::test]
    async fn read_response_communication_fail() {
        let mut realm_sender_mock = MockRealmSender::new();
        realm_sender_mock.expect_receive_response().returning(|_| {
            Err(RealmSenderError::ReceiveFail(
                JsonFramedError::StreamIsClosed(),
            ))
        });

        let mut realm_client_handler = create_realm_client_handler(None, None);
        realm_client_handler.sender = Some(Box::new(realm_sender_mock));
        assert!(matches!(
            realm_client_handler.read_response(TIMEOUT).await,
            Err(RealmClientError::CommunicationFail(_))
        ));
    }

    #[tokio::test]
    async fn handle_shutdown_response() {
        let mut realm_sender_mock = MockRealmSender::new();
        realm_sender_mock
            .expect_receive_response()
            .returning(|_| Err(RealmSenderError::Disconnection));

        let mut realm_client_handler = create_realm_client_handler(None, None);
        realm_client_handler.sender = Some(Box::new(realm_sender_mock));
        assert!(matches!(
            realm_client_handler.handle_shutdown_response().await,
            Ok(())
        ));
    }

    #[tokio::test]
    async fn handle_ip_response() {
        let mut realm_sender_mock = MockRealmSender::new();
        realm_sender_mock
            .expect_receive_response()
            .returning(|_| Ok(Response::IfAddrs(HashMap::new())));

        let mut realm_client_handler = create_realm_client_handler(None, None);
        realm_client_handler.sender = Some(Box::new(realm_sender_mock));
        assert!(matches!(
            realm_client_handler.handle_ip_response().await,
            Ok(hash_map) if hash_map.len() == 0
        ));
    }

    #[tokio::test]
    async fn handle_ip_invalid_response() {
        let mut realm_sender_mock = MockRealmSender::new();
        realm_sender_mock
            .expect_receive_response()
            .returning(|_| Ok(Response::Success()));

        let mut realm_client_handler = create_realm_client_handler(None, None);
        realm_client_handler.sender = Some(Box::new(realm_sender_mock));
        assert!(matches!(
            realm_client_handler.handle_ip_response().await,
            Err(RealmClientError::InvalidResponse(_))
        ));
    }

    #[tokio::test]
    async fn handle_shutdown_communication_fail() {
        let mut realm_client_handler = create_realm_client_handler(None, None);
        assert!(matches!(
            realm_client_handler.read_response(TIMEOUT).await,
            Err(RealmClientError::RealmConnectionFail(_))
        ));
    }

    #[tokio::test]
    async fn handle_shutdown_other_response() {
        let mut realm_sender_mock = MockRealmSender::new();
        realm_sender_mock
            .expect_receive_response()
            .returning(|_| Ok(Response::Success()));

        let mut realm_client_handler = create_realm_client_handler(None, None);
        realm_client_handler.sender = Some(Box::new(realm_sender_mock));
        assert!(matches!(
            realm_client_handler.handle_shutdown_response().await,
            Err(RealmClientError::InvalidResponse(_))
        ));
    }

    #[tokio::test]
    async fn handle_shutdown_error_response() {
        let mut realm_sender_mock = MockRealmSender::new();
        realm_sender_mock.expect_receive_response().returning(|_| {
            Ok(Response::Error(
                ProtocolError::InvalidRequest(String::new()),
            ))
        });

        let mut realm_client_handler = create_realm_client_handler(None, None);
        realm_client_handler.sender = Some(Box::new(realm_sender_mock));
        assert!(matches!(
            realm_client_handler.handle_shutdown_response().await,
            Err(RealmClientError::RealmDaemonError(_))
        ));
    }

    #[tokio::test]
    async fn read_response_connection_issue() {
        let mut realm_client_handler = create_realm_client_handler(None, None);
        assert!(matches!(
            realm_client_handler.read_response(TIMEOUT).await,
            Err(RealmClientError::RealmConnectionFail(_))
        ));
    }

    #[test]
    fn handle_success_response() {
        const RESP: Response = Response::Success();
        assert!(matches!(
            RealmClientHandler::handle_success_command(RESP),
            Ok(())
        ));
    }

    #[test]
    fn handle_not_success_response() {
        const RESP: Response = Response::ApplicationNotStarted();
        assert!(matches!(
            RealmClientHandler::handle_success_command(RESP),
            Err(RealmClientError::InvalidResponse(_))
        ))
    }

    #[test]
    fn handle_invalid_response() {
        const RESP: Response = Response::Error(ProtocolError::ApplicationsAlreadyProvisioned());
        assert!(matches!(
            RealmClientHandler::handle_invalid_response(RESP),
            RealmClientError::RealmDaemonError(_)
        ))
    }

    #[test]
    fn handle_invalid_other_response() {
        const RESP: Response = Response::ApplicationNotStarted();
        assert!(matches!(
            RealmClientHandler::handle_invalid_response(RESP),
            RealmClientError::InvalidResponse(_)
        ))
    }

    #[tokio::test]
    async fn start_application() {
        let app_uuid = Uuid::new_v4();
        let sender_mock = {
            let mut mock = MockRealmSender::new();
            let uuid_cpy = app_uuid.clone();
            mock.expect_send().return_once(move |req| match req {
                Request::StartApp(uuid) if uuid == uuid_cpy => Ok(()),
                _ => Err(RealmSenderError::Timeout),
            });
            mock.expect_receive_response()
                .returning(|_| Ok(Response::Success()));
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
                Request::StopApp(uuid) if uuid == uuid_cpy => Ok(()),
                _ => Err(RealmSenderError::Timeout),
            });
            mock.expect_receive_response()
                .returning(|_| Ok(Response::Success()));
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
    async fn kill_after_stop() {
        let app_uuid = Uuid::new_v4();
        let sender_mock = {
            let mut mock = MockRealmSender::new();
            mock.expect_send().returning(|_| Ok(()));
            mock.expect_send()
                .return_once(|_| Err(RealmSenderError::Timeout));
            mock.expect_receive_response()
                .returning(|_| Ok(Response::Success()));
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
                Request::KillApp(uuid) if uuid == uuid_cpy => Ok(()),
                _ => Err(RealmSenderError::Timeout),
            });
            mock.expect_receive_response()
                .returning(|_| Ok(Response::Success()));
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
                Request::Shutdown() => Ok(()),
                _ => Err(RealmSenderError::Timeout),
            });
            mock.expect_receive_response()
                .returning(|_| Err(RealmSenderError::Disconnection));
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
                Request::Reboot() => Ok(()),
                _ => Err(RealmSenderError::Timeout),
            });
            mock.expect_send().returning(|_| Ok(()));
            mock.expect_receive_response()
                .return_once(|_| Err(RealmSenderError::Disconnection));
            mock.expect_receive_response()
                .returning(|_| Ok(Response::Success()));
            mock
        };
        let mut realm_client_handler = create_realm_client_handler(None, None);
        realm_client_handler.sender = Some(Box::new(sender_mock));
        let cid = 0;
        assert!(realm_client_handler
            .reboot_realm(create_example_realm_provisioning_config(), cid)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn get_realms_interfaces() {
        const KEY: &str = "KEY";
        let sender_mock = {
            let mut mock = MockRealmSender::new();
            mock.expect_send().returning(|_| Ok(()));
            mock.expect_receive_response().returning(|_| {
                Ok(Response::IfAddrs({
                    let map = HashMap::from([(
                        String::from(KEY),
                        NetAddr {
                            address: std::net::IpAddr::V4(Ipv4Addr::LOCALHOST),
                            netmask: None,
                            destination: None,
                        },
                    )]);
                    map
                }))
            });
            mock
        };
        let mut realm_client_handler = create_realm_client_handler(None, None);
        realm_client_handler.sender = Some(Box::new(sender_mock));
        assert!(
            matches!(realm_client_handler.read_realm_ifs().await, Ok(hash_map) if hash_map[0].ip.is_loopback())
        );
    }

    #[tokio::test]
    async fn acknowledge_client_connection() {
        let mut realm_client_handler = create_realm_client_handler(None, None);
        let cid = 0;
        assert!(realm_client_handler.sender.is_none());
        assert!(realm_client_handler
            .provision_applications(create_example_realm_provisioning_config(), cid)
            .await
            .is_ok());
        assert!(realm_client_handler.sender.is_some());
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
            .provision_applications(create_example_realm_provisioning_config(), cid)
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
            realm_sender.expect_send().returning(|_| Ok(()));
            realm_sender
                .expect_receive_response()
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
        RealmClientHandler::new(
            Arc::new(Mutex::new(realm_connector)),
            Duration::from_secs(0),
            Duration::from_secs(0),
            Arc::new(CancellationToken::new()),
        )
    }
}
