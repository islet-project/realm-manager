use crate::managers::application::{Application, ApplicationError};
use crate::managers::realm::{Realm, RealmError};
use crate::managers::realm_configuration::RealmConfig;
use crate::managers::warden::{Warden, WardenError};

use async_trait::async_trait;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::{net::UnixStream, select};
use tokio_util::sync::CancellationToken;
use utils::serde::json_framed::{JsonFramed, JsonFramedError};
use uuid::Uuid;
use warden_client::warden::{WardenCommand, WardenDaemonError, WardenResponse};

#[derive(Debug, Error, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum ClientError {
    #[error("Failed to read request. Error: {message}")]
    InvalidRequest { message: String },
    #[error("Warden error occured: {0}")]
    WardenDaemonError(WardenError),
    #[error("Realm error occured: {0}")]
    RealmManagerError(RealmError),
    #[error("Application error occured: {0}")]
    ApplicationError(ApplicationError),
    #[error("Failed to send response. Error: {message}")]
    SendingResponseFail { message: String },
}

#[async_trait]
pub trait Client {
    async fn handle_connection(
        warden: Arc<Mutex<Box<dyn Warden + Send + Sync>>>,
        stream: UnixStream,
        token: Arc<CancellationToken>,
    ) -> Result<(), ClientError>;
}

pub struct ClientHandler {
    warden: Arc<Mutex<Box<dyn Warden + Send + Sync>>>,
    communicator: JsonFramed<UnixStream, WardenCommand, WardenResponse>,
    token: Arc<CancellationToken>,
}

impl ClientHandler {
    pub async fn handle_client_requests(&mut self) -> Result<(), ClientError> {
        loop {
            select! {
                command_result = self.communicator.recv() => {
                    let command = match command_result {
                        Ok(command) => command,
                        Err(JsonFramedError::StreamIsClosed()) => {
                            info!("Client has disconnected.");
                            break;
                        },
                        Err(_) => {
                            return self.communicator.send(WardenResponse::Error { warden_error: WardenDaemonError::ReadingRequestFail }).await.map_err(|err|ClientError::SendingResponseFail { message: err.to_string() });
                        },
                    };
                    self.handle_client_command(command).await?;
                }
                _ = self.token.cancelled() => {
                    break;
                }
            }
        }
        Ok(())
    }

    async fn handle_client_command(&mut self, command: WardenCommand) -> Result<(), ClientError> {
        let handle_result = self.handle_command(command).await.map_err(|client_err| {
            WardenDaemonError::WardenDaemonFail {
                message: client_err.to_string(),
            }
        });
        self.send_handle_result(handle_result).await
    }

    async fn send_handle_result(
        &mut self,
        handle_result: Result<WardenResponse, WardenDaemonError>,
    ) -> Result<(), ClientError> {
        let response = create_response(handle_result);
        self.communicator
            .send(response)
            .await
            .map_err(|err| ClientError::SendingResponseFail {
                message: err.to_string(),
            })
    }

    async fn handle_command(
        &mut self,
        client_command: WardenCommand,
    ) -> Result<WardenResponse, ClientError> {
        match client_command {
            WardenCommand::StartRealm { uuid } => {
                info!("Starting realm: {uuid}.");
                let realm: Arc<Mutex<Box<dyn Realm + Send + Sync>>> = self.get_realm(&uuid).await?;
                realm
                    .lock()
                    .await
                    .start()
                    .await
                    .map_err(ClientError::RealmManagerError)?;
                info!("Started realm: {uuid}.");
                Ok(WardenResponse::Ok)
            }
            WardenCommand::CreateRealm { config } => {
                info!("Creating realm.");
                let uuid = self
                    .warden
                    .lock()
                    .await
                    .create_realm(RealmConfig::from(config))
                    .await
                    .map_err(ClientError::WardenDaemonError)?;
                info!("Realm: {uuid} created.");
                Ok(WardenResponse::CreatedRealm { uuid })
            }
            WardenCommand::StopRealm { uuid } => {
                info!("Stopping realm: {uuid}.");
                let realm = self.get_realm(&uuid).await?;
                realm
                    .lock()
                    .await
                    .stop()
                    .map_err(ClientError::RealmManagerError)?;
                info!("Realm: {uuid} stopped.");
                Ok(WardenResponse::Ok)
            }
            WardenCommand::DestroyRealm { uuid } => {
                info!("Destroying realm: {uuid}.");
                self.warden
                    .lock()
                    .await
                    .destroy_realm(&uuid)
                    .await
                    .map_err(ClientError::WardenDaemonError)?;
                info!("Realm: {uuid} destroyed.");
                Ok(WardenResponse::Ok)
            }
            WardenCommand::RebootRealm { uuid } => {
                info!("Rebooting realm: {uuid}.");
                let realm = self.get_realm(&uuid).await?;
                realm
                    .lock()
                    .await
                    .reboot()
                    .await
                    .map_err(ClientError::RealmManagerError)?;
                info!("Realm: {uuid} rebooted.");
                Ok(WardenResponse::Ok)
            }
            WardenCommand::InspectRealm { uuid } => {
                info!("Inspecting realm: {uuid}.");
                let warden = self.warden.lock().await;
                let realm_data = warden
                    .inspect_realm(&uuid)
                    .await
                    .map_err(ClientError::WardenDaemonError)?;
                info!("Realm: {uuid} inspected.");
                Ok(WardenResponse::InspectedRealm {
                    description: realm_data.into(),
                })
            }
            WardenCommand::ListRealms => {
                info!("Listing realms.");
                let listed_realms = self
                    .warden
                    .lock()
                    .await
                    .list_realms()
                    .await
                    .into_iter()
                    .map(|description| description.into())
                    .collect();
                info!("Realms listed.");
                Ok(WardenResponse::ListedRealms {
                    realms_description: listed_realms,
                })
            }
            WardenCommand::CreateApplication { realm_uuid, config } => {
                info!("Creating application in realm: {realm_uuid}.");
                let realm = self.get_realm(&realm_uuid).await?;
                let application_uuid = realm
                    .lock()
                    .await
                    .create_application(config.into())
                    .map_err(ClientError::RealmManagerError)?;
                info!("Created application with id: {application_uuid} in realm: {realm_uuid}.");
                Ok(WardenResponse::CreatedApplication {
                    uuid: application_uuid,
                })
            }
            WardenCommand::StartApplication {
                realm_uuid,
                application_uuid,
            } => {
                info!("Starting application: {application_uuid} in realm: {realm_uuid}.");
                let application = self.get_application(&realm_uuid, &application_uuid).await?;
                application
                    .lock()
                    .await
                    .start()
                    .await
                    .map_err(ClientError::ApplicationError)?;
                info!("Started application: {application_uuid} in realm: {realm_uuid}.");
                Ok(WardenResponse::Ok)
            }
            WardenCommand::StopApplication {
                realm_uuid,
                application_uuid,
            } => {
                info!("Stopping application: {application_uuid} in realm: {realm_uuid}.");
                let application = self.get_application(&realm_uuid, &application_uuid).await?;
                application
                    .lock()
                    .await
                    .stop()
                    .await
                    .map_err(ClientError::ApplicationError)?;
                info!("Stopped application: {application_uuid} in realm: {realm_uuid}.");
                Ok(WardenResponse::Ok)
            }
            WardenCommand::UpdateApplication {
                realm_uuid,
                application_uuid,
                config,
            } => {
                info!("Starting application: {application_uuid} in realm: {realm_uuid}.");
                self.get_realm(&realm_uuid)
                    .await?
                    .lock()
                    .await
                    .update_application(&application_uuid, config.into())
                    .await
                    .map_err(ClientError::RealmManagerError)?;
                info!("Started application: {application_uuid} in realm: {realm_uuid}.");
                Ok(WardenResponse::Ok)
            }
        }
    }

    async fn get_realm(
        &self,
        uuid: &Uuid,
    ) -> Result<Arc<Mutex<Box<dyn Realm + Send + Sync>>>, ClientError> {
        self.warden
            .lock()
            .await
            .get_realm(uuid)
            .map_err(ClientError::WardenDaemonError)
    }

    async fn get_application(
        &self,
        realm_uuid: &Uuid,
        application_uuid: &Uuid,
    ) -> Result<Arc<Mutex<Box<dyn Application + Send + Sync>>>, ClientError> {
        self.get_realm(realm_uuid)
            .await?
            .lock()
            .await
            .get_application(application_uuid)
            .map_err(ClientError::RealmManagerError)
    }
}

fn create_response(handle_result: Result<WardenResponse, WardenDaemonError>) -> WardenResponse {
    match handle_result {
        Ok(response) => response,
        Err(warden_error) => {
            error!(
                "Error has occured while handling client command: {}",
                warden_error
            );
            WardenResponse::Error { warden_error }
        }
    }
}

#[async_trait]
impl Client for ClientHandler {
    async fn handle_connection(
        warden: Arc<Mutex<Box<dyn Warden + Send + Sync>>>,
        stream: UnixStream,
        token: Arc<CancellationToken>,
    ) -> Result<(), ClientError> {
        let mut handler = ClientHandler {
            warden,
            communicator: JsonFramed::<UnixStream, WardenCommand, WardenResponse>::new(stream),
            token,
        };
        handler.handle_client_requests().await
    }
}

#[cfg(test)]
mod test {
    use crate::managers::{
        application::Application,
        realm::{Realm, RealmData, RealmError, State},
        warden::WardenError,
    };
    use crate::test_utilities::{
        create_example_realm_description, create_example_uuid, MockApplication, MockRealm,
        MockWardenDaemon,
    };
    use parameterized::parameterized;
    use std::{path::PathBuf, sync::Arc};
    use tokio::{net::UnixStream, sync::Mutex};
    use tokio_util::sync::CancellationToken;
    use utils::serde::json_framed::JsonFramed;
    use uuid::Uuid;
    use warden_client::{
        applciation::ApplicationConfig,
        realm::{CpuConfig, KernelConfig, MemoryConfig, NetworkConfig, RealmConfig},
        warden::{WardenCommand, WardenResponse},
    };

    use super::{ClientError, ClientHandler};

    #[tokio::test]
    async fn handle_requests_and_disconnect() {
        const INPUT: WardenCommand = WardenCommand::ListRealms;
        let (mut client_socket, mut client_handler) = create_client_handler(None).await;
        let task = tokio::spawn(async move {
            client_socket.send(INPUT).await.unwrap();
            client_socket.recv().await.unwrap();
        });
        assert_eq!(client_handler.handle_client_requests().await, Ok(()));
        task.await.unwrap();
    }

    #[tokio::test]
    async fn handle_requests_token_cancellation() {
        let (mut _client_socket, mut client_handler) = create_client_handler(None).await;
        client_handler.token.cancel();
        assert_eq!(client_handler.handle_client_requests().await, Ok(()));
    }

    #[tokio::test]
    async fn get_realm() {
        let mut warden_daemon = MockWardenDaemon::new();
        warden_daemon
            .expect_get_realm()
            .return_once(|_| Ok(Arc::new(Mutex::new(Box::new(MockRealm::new())))));
        let (mut _client_socket, client_handler) = create_client_handler(Some(warden_daemon)).await;
        assert!(client_handler.get_realm(&Uuid::new_v4()).await.is_ok());
    }

    #[tokio::test]
    async fn get_realm_fail() {
        let mut warden_daemon = MockWardenDaemon::new();
        let uuid = Uuid::new_v4();
        warden_daemon
            .expect_get_realm()
            .return_once(|uuid| Err(WardenError::NoSuchRealm(*uuid)));
        let (mut _client_socket, client_handler) = create_client_handler(Some(warden_daemon)).await;
        assert_eq!(
            client_handler.get_realm(&uuid).await.err().unwrap(),
            ClientError::WardenDaemonError(WardenError::NoSuchRealm(uuid))
        );
    }

    #[tokio::test]
    async fn get_application() {
        let mut realm_manager = MockRealm::new();
        realm_manager
            .expect_get_application()
            .return_once(|_| Ok(Arc::new(Mutex::new(Box::new(MockApplication::new())))));
        let mut warden_daemon = MockWardenDaemon::new();
        warden_daemon
            .expect_get_realm()
            .return_once(|_| Ok(Arc::new(Mutex::new(Box::new(realm_manager)))));
        let (mut _client_socket, client_handler) = create_client_handler(Some(warden_daemon)).await;
        assert!(client_handler
            .get_application(&Uuid::new_v4(), &Uuid::new_v4())
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn get_application_realm_fail() {
        let mut warden_daemon = MockWardenDaemon::new();
        let uuid = Uuid::new_v4();
        warden_daemon
            .expect_get_realm()
            .return_once(|uuid| Err(WardenError::NoSuchRealm(*uuid)));
        let (mut _client_socket, client_handler) = create_client_handler(Some(warden_daemon)).await;
        assert_eq!(
            client_handler
                .get_application(&uuid, &Uuid::new_v4())
                .await
                .err()
                .unwrap(),
            ClientError::WardenDaemonError(WardenError::NoSuchRealm(uuid))
        );
    }

    #[tokio::test]
    async fn get_application_fail() {
        let mut realm_manager = MockRealm::new();
        realm_manager
            .expect_get_application()
            .return_once(|uuid| Err(RealmError::ApplicationMissing(*uuid)));
        let mut warden_daemon = MockWardenDaemon::new();
        warden_daemon
            .expect_get_realm()
            .return_once(|_| Ok(Arc::new(Mutex::new(Box::new(realm_manager)))));
        let (mut _client_socket, client_handler) = create_client_handler(Some(warden_daemon)).await;
        let app_uuid = Uuid::new_v4();
        assert_eq!(
            client_handler
                .get_application(&Uuid::new_v4(), &app_uuid)
                .await
                .err()
                .unwrap(),
            ClientError::RealmManagerError(RealmError::ApplicationMissing(app_uuid))
        );
    }

    #[tokio::test]
    #[parameterized(input = {
        (WardenCommand::CreateRealm { config: create_example_client_realm_config() }, WardenResponse::CreatedRealm{uuid: create_example_uuid()}),
        (WardenCommand::StartRealm { uuid: create_example_uuid()}, WardenResponse::Ok),
        (WardenCommand::StopRealm { uuid: create_example_uuid() }, WardenResponse::Ok),
        (WardenCommand::DestroyRealm { uuid: create_example_uuid() }, WardenResponse::Ok),
        (WardenCommand::RebootRealm { uuid: create_example_uuid() }, WardenResponse::Ok),
        (WardenCommand::InspectRealm { uuid: create_example_uuid() }, WardenResponse::InspectedRealm { description: create_example_realm_description().into() }),
        (WardenCommand::ListRealms, WardenResponse::ListedRealms { realms_description: vec![create_example_realm_description().into()] }),
        (WardenCommand::CreateApplication { realm_uuid: create_example_uuid(), config: create_example_client_app_config() }, WardenResponse::CreatedApplication { uuid: create_example_uuid() }),
        (WardenCommand::StartApplication { realm_uuid: create_example_uuid(), application_uuid: create_example_uuid() }, WardenResponse::Ok),
        (WardenCommand::StopApplication { realm_uuid: create_example_uuid(), application_uuid: create_example_uuid() }, WardenResponse::Ok),
        (WardenCommand::UpdateApplication { realm_uuid: create_example_uuid(), application_uuid: create_example_uuid(), config: create_example_client_app_config() }, WardenResponse::Ok),
    })]
    async fn handle_request(input: (WardenCommand, WardenResponse)) {
        let (command, response) = input;
        let (mut receive_socket, mut client_handler) = create_client_handler(None).await;
        let reader = tokio::spawn(async move { receive_socket.recv().await.unwrap() });
        assert!(client_handler.handle_client_command(command).await.is_ok());
        assert_eq!(reader.await.unwrap(), response);
    }

    async fn create_client_handler(
        warden_daemon: Option<MockWardenDaemon>,
    ) -> (
        JsonFramed<UnixStream, WardenResponse, WardenCommand>,
        ClientHandler,
    ) {
        let application_manager: Arc<Mutex<Box<dyn Application + Send + Sync>>> =
            Arc::new(Mutex::new(Box::new({
                let mut realm_manager = MockApplication::new();
                realm_manager.expect_start().returning(|| Ok(()));
                realm_manager.expect_stop().returning(|| Ok(()));
                realm_manager.expect_update().returning(|_| ());
                realm_manager
            })));

        let realm_manager: Arc<Mutex<Box<dyn Realm + Send + Sync>>> =
            Arc::new(Mutex::new(Box::new({
                let mut realm_manager = MockRealm::new();
                realm_manager.expect_start().returning(|| Ok(()));
                realm_manager.expect_stop().returning(|| Ok(()));
                realm_manager.expect_reboot().returning(|| Ok(()));
                realm_manager
                    .expect_get_realm_data()
                    .returning(|| RealmData {
                        state: State::Halted,
                    });
                realm_manager
                    .expect_get_application()
                    .return_once(|_| Ok(application_manager));
                realm_manager
                    .expect_update_application()
                    .returning(|_, _| Ok(()));
                realm_manager
                    .expect_create_application()
                    .returning(|_| Ok(create_example_uuid()));
                realm_manager
            })));

        let warden_daemon = warden_daemon.unwrap_or({
            let mut warden_daemon = MockWardenDaemon::new();
            warden_daemon
                .expect_create_realm()
                .returning(|_| Ok(create_example_uuid()));
            warden_daemon.expect_destroy_realm().returning(|_| Ok(()));
            warden_daemon
                .expect_list_realms()
                .returning(|| vec![create_example_realm_description()]);
            warden_daemon
                .expect_inspect_realm()
                .returning(|_| Ok(create_example_realm_description()));
            warden_daemon
                .expect_get_realm()
                .return_once(|_| Ok(realm_manager));
            warden_daemon
        });
        let (receive_stream, client_stream) = UnixStream::pair().unwrap();
        (
            JsonFramed::<UnixStream, WardenResponse, WardenCommand>::new(receive_stream),
            ClientHandler {
                warden: Arc::new(Mutex::new(Box::new(warden_daemon))),
                communicator: JsonFramed::<UnixStream, WardenCommand, WardenResponse>::new(
                    client_stream,
                ),
                token: Arc::new(CancellationToken::new()),
            },
        )
    }

    fn create_example_client_realm_config() -> RealmConfig {
        RealmConfig {
            machine: String::new(),
            cpu: CpuConfig {
                cpu: String::new(),
                cores_number: 0,
            },
            memory: MemoryConfig { ram_size: 0 },
            network: NetworkConfig {
                vsock_cid: 0,
                tap_device: String::new(),
                mac_address: String::new(),
                hardware_device: None,
                remote_terminal_uri: None,
            },
            kernel: KernelConfig {
                kernel_path: PathBuf::new(),
            },
        }
    }

    fn create_example_client_app_config() -> ApplicationConfig {
        ApplicationConfig {}
    }
}
