use async_trait::async_trait;
use log::{error, info, trace};
use serde::{Deserialize, Serialize};
use std::io;
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;
use tokio::{net::UnixStream, select};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::managers::application::{Application, ApplicationConfig, ApplicationError};
use crate::managers::realm::{Realm, RealmDescription, RealmError};
use crate::managers::realm_configuration::RealmConfig;

use crate::managers::warden::{Warden, WardenError};

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientCommand {
    CreateRealm {
        config: RealmConfig,
    },
    StartRealm {
        uuid: Uuid,
    },
    StopRealm {
        uuid: Uuid,
    },
    RebootRealm {
        uuid: Uuid,
    },
    DestroyRealm {
        uuid: Uuid,
    },
    InspectRealm {
        uuid: Uuid,
    },
    ListRealms,
    CreateApplication {
        realm_uuid: Uuid,
        config: ApplicationConfig,
    },
    StartApplication {
        realm_uuid: Uuid,
        application_uuid: Uuid,
    },
    StopApplication {
        realm_uuid: Uuid,
        application_uuid: Uuid,
    },
    UpdateApplication {
        realm_uuid: Uuid,
        application_uuid: Uuid,
        config: ApplicationConfig,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientReponse {
    Ok,
    CreatedRealm { uuid: Uuid },
    InspectedRealm(RealmDescription),
    ListedRealms(Vec<RealmDescription>),
    Error(ClientError),
}

#[derive(Debug, Error, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum ClientError {
    #[error("Failed to read request!")]
    ReadingRequestFail,
    #[error("Can't recognise a command!")]
    UnknownCommand { length: usize },
    #[error("Provided Uuid is invalid!")]
    InvalidUuid,
    #[error("Can't serialize a response!")]
    ParsingResponseFail,
    #[error("Warden error occured: {0}!")]
    WardenDaemonError(WardenError),
    #[error("Realm error occured: {0}!")]
    RealmManagerError(RealmError),
    #[error("Application error occured: {0}!")]
    ApplicationError(ApplicationError),
    #[error("Failed to send response!")]
    SendingResponseFail,
}

#[async_trait]
pub trait Client {
    async fn handle_connection(
        warden: Arc<Mutex<Box<dyn Warden + Send + Sync>>>,
        socket: UnixStream,
        token: Arc<CancellationToken>,
    ) -> Result<(), ClientError>;
}

pub struct ClientHandler {
    warden: Arc<Mutex<Box<dyn Warden + Send + Sync>>>,
    socket: BufReader<UnixStream>,
    token: Arc<CancellationToken>,
}

impl ClientHandler {
    pub async fn handle_requests(&mut self) -> Result<(), ClientError> {
        loop {
            // TODO! Refactor command reading using tokio_serde
            let mut request_data = String::new();
            select! {
                readed_bytes = self.socket.read_line(&mut request_data) => {
                    if let Err(err) = self.handle_request(readed_bytes, request_data).await {
                        match err {
                            ClientError::UnknownCommand{length: 0} => { break; }, // Client disconnected
                            _ => {
                                error!("Error has occured while handling client command: {}", err);
                                let _ = self.socket.write_all(&serde_json::to_vec(&err).map_err(|_|ClientError::ParsingResponseFail)?).await;
                            }
                        }
                    }
                }
                _ = self.token.cancelled() => {
                    break;
                }
            }
        }
        Ok(())
    }

    async fn handle_request(
        &mut self,
        readed_bytes: Result<usize, io::Error>,
        request_data: String,
    ) -> Result<(), ClientError> {
        let _ = readed_bytes.map_err(|_| ClientError::ReadingRequestFail)?;
        trace!("Received message: {request_data}");
        let command = self.resolve_command(request_data)?;
        let response = self.handle_command(command).await?;
        self.socket
            .write_all(
                &serde_json::to_vec(&response).map_err(|_| ClientError::ParsingResponseFail)?,
            )
            .await
            .map_err(|_| ClientError::SendingResponseFail)
    }

    async fn handle_command(
        &mut self,
        client_command: ClientCommand,
    ) -> Result<ClientReponse, ClientError> {
        match client_command {
            ClientCommand::StartRealm { uuid } => {
                info!("Starting realm: {uuid}");
                let realm = self.get_realm(&uuid).await?;
                realm
                    .lock()
                    .await
                    .start()
                    .await
                    .map_err(ClientError::RealmManagerError)?;
                info!("Started realm: {uuid}");
                Ok(ClientReponse::Ok)
            }
            ClientCommand::CreateRealm { config } => {
                info!("Creating realm!");
                let uuid = self
                    .warden
                    .lock()
                    .await
                    .create_realm(config)
                    .map_err(ClientError::WardenDaemonError)?;
                info!("Realm: {uuid} created!");
                Ok(ClientReponse::CreatedRealm { uuid })
            }
            ClientCommand::StopRealm { uuid } => {
                info!("Stopping realm: {uuid}!");
                let realm = self.get_realm(&uuid).await?;
                realm
                    .lock()
                    .await
                    .stop()
                    .map_err(ClientError::RealmManagerError)?;
                info!("Realm: {uuid} stopped!");
                Ok(ClientReponse::Ok)
            }
            ClientCommand::DestroyRealm { uuid } => {
                info!("Destroying realm: {uuid}!");
                self.warden
                    .lock()
                    .await
                    .destroy_realm(uuid)
                    .await
                    .map_err(ClientError::WardenDaemonError)?;
                info!("Realm: {uuid} destroyed!");
                Ok(ClientReponse::Ok)
            }
            ClientCommand::RebootRealm { uuid } => {
                info!("Rebooting realm: {uuid}!");
                let realm = self.get_realm(&uuid).await?;
                realm
                    .lock()
                    .await
                    .reboot()
                    .await
                    .map_err(ClientError::RealmManagerError)?;
                info!("Realm: {uuid} rebooted!");
                Ok(ClientReponse::Ok)
            }
            ClientCommand::InspectRealm { uuid } => {
                info!("Inspecting realm: {uuid}!");
                let warden = self.warden.lock().await;
                let realm_data = warden
                    .inspect_realm(uuid)
                    .await
                    .map_err(ClientError::WardenDaemonError)?;
                info!("Realm: {uuid} inspected!");
                Ok(ClientReponse::InspectedRealm(realm_data))
            }
            ClientCommand::ListRealms => {
                info!("Listing realms!");
                let listed_realms = self.warden.lock().await.list_realms().await;
                info!("Realms listed!");
                Ok(ClientReponse::ListedRealms(listed_realms))
            }
            ClientCommand::CreateApplication { realm_uuid, config } => {
                info!("Creating application in realm: {realm_uuid}!");
                let realm = self.get_realm(&realm_uuid).await?;
                let application_uuid = realm
                    .lock()
                    .await
                    .create_application(config)
                    .await
                    .map_err(ClientError::RealmManagerError)?;
                info!("Created application with id: {application_uuid} in realm: {realm_uuid}!");
                Ok(ClientReponse::Ok)
            }
            ClientCommand::StartApplication {
                realm_uuid,
                application_uuid,
            } => {
                info!("Starting application: {application_uuid} in realm: {realm_uuid}!");
                let application = self.get_application(&realm_uuid, &application_uuid).await?;
                application
                    .lock()
                    .await
                    .start()
                    .await
                    .map_err(ClientError::ApplicationError)?;
                info!("Started application: {application_uuid} in realm: {realm_uuid}!");
                Ok(ClientReponse::Ok)
            }
            ClientCommand::StopApplication {
                realm_uuid,
                application_uuid,
            } => {
                info!("Stopping application: {application_uuid} in realm: {realm_uuid}!");
                let application = self.get_application(&realm_uuid, &application_uuid).await?;
                application
                    .lock()
                    .await
                    .stop()
                    .await
                    .map_err(ClientError::ApplicationError)?;
                info!("Stopped application: {application_uuid} in realm: {realm_uuid}!");
                Ok(ClientReponse::Ok)
            }
            ClientCommand::UpdateApplication {
                realm_uuid,
                application_uuid,
                config,
            } => {
                info!("Starting application: {application_uuid} in realm: {realm_uuid}!");
                self.get_realm(&realm_uuid)
                    .await?
                    .lock()
                    .await
                    .update_application(application_uuid, config)
                    .await
                    .map_err(ClientError::RealmManagerError)?;
                info!("Started application: {application_uuid} in realm: {realm_uuid}!");
                Ok(ClientReponse::Ok)
            }
        }
    }

    fn resolve_command(&self, serialized_command: String) -> Result<ClientCommand, ClientError> {
        let command: ClientCommand =
            serde_json::from_str(&serialized_command).map_err(|_| ClientError::UnknownCommand {
                length: serialized_command.len(),
            })?;
        Ok(command)
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
            .get_application(*application_uuid)
            .await
            .map_err(ClientError::RealmManagerError)
    }
}

#[async_trait]
impl Client for ClientHandler {
    async fn handle_connection(
        warden: Arc<Mutex<Box<dyn Warden + Send + Sync>>>,
        socket: UnixStream,
        token: Arc<CancellationToken>,
    ) -> Result<(), ClientError> {
        let mut handler = ClientHandler {
            warden,
            socket: BufReader::new(socket),
            token,
        };
        handler.handle_requests().await
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
        create_example_app_config, create_example_realm_config, create_example_realm_description,
        create_example_uuid, MockApplication, MockRealm, MockWardenDaemon,
    };
    use parameterized::parameterized;
    use std::sync::Arc;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt, BufReader},
        net::UnixStream,
        sync::Mutex,
    };
    use tokio_util::sync::CancellationToken;
    use uuid::Uuid;

    use super::{ClientCommand, ClientError, ClientHandler, ClientReponse};

    #[tokio::test]
    async fn handle_requests_and_disconnect() {
        const INPUT: ClientCommand = ClientCommand::ListRealms;
        let (mut client_socket, mut client_handler) = create_client_handler(None).await;
        let task = tokio::spawn(async move {
            client_socket
                .write_all(&serde_json::to_vec(&INPUT).unwrap())
                .await
                .unwrap();
            client_socket.write(&vec![0xA]).await.unwrap(); //  New line char
            let mut buffer = [0; 2048];
            client_socket.read(&mut buffer).await.unwrap();
            client_socket.shutdown().await.unwrap(); // Testing shutdown
        });
        assert_eq!(client_handler.handle_requests().await, Ok(()));
        task.await.unwrap();
    }

    #[tokio::test]
    async fn handle_requests_token_cancellation() {
        let (mut _client_socket, mut client_handler) = create_client_handler(None).await;
        client_handler.token.cancel();
        assert_eq!(client_handler.handle_requests().await, Ok(()));
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
            .return_once(|uuid| Err(RealmError::ApplicationMissing(uuid)));
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
    async fn handle_requests_invalid_command() {
        const MESSAGE: &str = "{}";
        let mut data = serde_json::to_vec(MESSAGE).unwrap();
        data.push(0xA); // new line
        let length = data.len();
        let (mut client_socket, mut client_handler) = create_client_handler(None).await;
        let task = tokio::spawn(async move {
            client_socket.write_all(&data).await.unwrap();
            let mut buffer = [0; 2048];
            client_socket.read(&mut buffer).await.unwrap();
            client_socket.shutdown().await.unwrap();
            buffer
        });
        assert_eq!(client_handler.handle_requests().await, Ok(()));
        let mut request_response = std::str::from_utf8(&task.await.unwrap())
            .unwrap()
            .to_string();
        request_response.retain(|c| c != '\0');
        assert_eq!(
            request_response,
            serde_json::to_string(&ClientError::UnknownCommand { length }).unwrap()
        );
    }

    #[tokio::test]
    #[parameterized(input = {
        (ClientCommand::CreateRealm { config: create_example_realm_config() }, ClientReponse::CreatedRealm{uuid: create_example_uuid()}),
        (ClientCommand::StartRealm { uuid: create_example_uuid()}, ClientReponse::Ok),
        (ClientCommand::StopRealm { uuid: create_example_uuid() }, ClientReponse::Ok),
        (ClientCommand::DestroyRealm { uuid: create_example_uuid() }, ClientReponse::Ok),
        (ClientCommand::RebootRealm { uuid: create_example_uuid() }, ClientReponse::Ok),
        (ClientCommand::InspectRealm { uuid: create_example_uuid() }, ClientReponse::InspectedRealm(create_example_realm_description())),
        (ClientCommand::ListRealms, ClientReponse::ListedRealms(vec![create_example_realm_description()])),
        (ClientCommand::CreateApplication { realm_uuid: create_example_uuid(), config: create_example_app_config() }, ClientReponse::Ok),
        (ClientCommand::StartApplication { realm_uuid: create_example_uuid(), application_uuid: create_example_uuid() }, ClientReponse::Ok),
        (ClientCommand::StopApplication { realm_uuid: create_example_uuid(), application_uuid: create_example_uuid() }, ClientReponse::Ok),
        (ClientCommand::UpdateApplication { realm_uuid: create_example_uuid(), application_uuid: create_example_uuid(), config: create_example_app_config() }, ClientReponse::Ok),
    })]
    async fn handle_request(input: (ClientCommand, ClientReponse)) {
        let (request, response) = input;
        let (mut receive_socket, mut client_handler) = create_client_handler(None).await;
        let reader = tokio::spawn(async move {
            let mut buffer = [0; 2048];
            receive_socket.read(&mut buffer).await.unwrap();
            buffer
        });
        assert_eq!(
            client_handler
                .handle_request(Ok(0), serde_json::to_string(&request).unwrap())
                .await,
            Ok(())
        );
        let mut request_response = std::str::from_utf8(&reader.await.unwrap())
            .unwrap()
            .to_string();
        request_response.retain(|c| c != '\0');
        assert_eq!(request_response, serde_json::to_string(&response).unwrap());
    }

    async fn create_client_handler(
        warden_daemon: Option<MockWardenDaemon>,
    ) -> (UnixStream, ClientHandler) {
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
        let (receive_socket, client_socket) = UnixStream::pair().unwrap();
        (
            receive_socket,
            ClientHandler {
                warden: Arc::new(Mutex::new(Box::new(warden_daemon))),
                socket: BufReader::new(client_socket),
                token: Arc::new(CancellationToken::new()),
            },
        )
    }
}
