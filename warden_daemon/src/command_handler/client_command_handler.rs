use async_trait::async_trait;
use log::{error, info, trace};
use serde::{Deserialize, Serialize};
use std::io;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio::{net::UnixStream, select};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::managers::application::ApplicationConfig;
use crate::managers::realm::RealmError;
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
    ListRealms,
    InspectRealm {
        uuid: Uuid,
    },
    CreateApplication {
        realm_uuid: Uuid,
        config: ApplicationConfig,
    },
    StopApplication {
        realm_uuid: Uuid,
        application_uuid: Uuid,
    },
    StartApplication {
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
    CreatedRealm { uuid: String },
    Error(ClientError),
}

#[derive(Debug, Error, Serialize, Deserialize)]
pub enum ClientError {
    #[error("Failed to read request!")]
    ReadingRequestFail,
    #[error("Can't recognise a command!")]
    UnknownCommand { size: usize },
    #[error("Provided Uuid is invalid!")]
    InvalidUuid,
    #[error("Can't serialize a response")]
    ParsingResponseFail,
    #[error("Warden error occured!")]
    WardenDaemonError(WardenError),
    #[error("Realm error occured!")]
    RealmManagerError(RealmError),
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
            let mut request_data = String::new();
            select! {
                readed_bytes = self.socket.read_line(&mut request_data) => {
                    match self.handle_request(readed_bytes, request_data).await {
                        Err(err) => match err {
                            ClientError::UnknownCommand{size: 0} => { break; }, // Client disconnected
                            _ => {
                                error!("Error has occured: {}", err);
                                let _ = self.socket.write_all(&serde_json::to_vec(&err).map_err(|_|ClientError::ParsingResponseFail)?).await;
                            }
                        },
                        _ => {},
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
                let _ = self
                    .warden
                    .lock()
                    .await
                    .as_mut()
                    .get_realm(&uuid)
                    .map_err(|err| ClientError::WardenDaemonError(err))?
                    .lock()
                    .await
                    .start()
                    .await
                    .map_err(|err| ClientError::RealmManagerError(err))?;
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
                    .map_err(|err| ClientError::WardenDaemonError(err))?;
                info!("Realm: {uuid} created!");
                Ok(ClientReponse::Ok)
            }
            ClientCommand::StopRealm { uuid } => {
                info!("Stopping realm: {uuid}!");
                let _ = self
                    .warden
                    .lock()
                    .await
                    .get_realm(&uuid)
                    .map_err(|err| ClientError::WardenDaemonError(err))?
                    .lock()
                    .await
                    .stop();
                info!("Realm: {uuid} stopped!");
                Ok(ClientReponse::Ok)
            }
            ClientCommand::DestroyRealm { uuid } => {
                info!("Destroying realm: {uuid}!");
                let _ = self.warden.lock().await.destroy_realm(uuid);
                info!("Realm: {uuid} destroyed!");
                Ok(ClientReponse::Ok)
            }
            ClientCommand::RebootRealm { uuid } => {
                info!("Rebooting realm: {uuid}!");
                let _realm = self
                    .warden
                    .lock()
                    .await
                    .get_realm(&uuid)
                    .map_err(|err| ClientError::WardenDaemonError(err))?;
                info!("Realm: {uuid} rebooted!");
                Ok(ClientReponse::Ok)
            }
            ClientCommand::InspectRealm { uuid } => {
                info!("Inspecting realm: {uuid}!");
                info!("Realm: {uuid} inspected!");
                Ok(ClientReponse::Ok)
            }
            ClientCommand::ListRealms => {
                info!("Listing realms!");
                info!("Realms listed!");
                Ok(ClientReponse::Ok)
            }
            ClientCommand::CreateApplication {
                realm_uuid,
                config: _,
            } => {
                info!("Creating application in realm: {realm_uuid}!");
                let application_uuid = Uuid::new_v4();
                info!("Created application with id: {application_uuid} in realm: {realm_uuid}!");
                Ok(ClientReponse::Ok)
            }
            ClientCommand::StartApplication {
                realm_uuid,
                application_uuid,
            } => {
                info!("Starting application: {application_uuid} in realm: {realm_uuid}!");
                info!("Started application: {application_uuid} in realm: {realm_uuid}!");
                Ok(ClientReponse::Ok)
            }
            ClientCommand::StopApplication {
                realm_uuid,
                application_uuid,
            } => {
                info!("Stopping application: {application_uuid} in realm: {realm_uuid}!");
                info!("Stopped application: {application_uuid} in realm: {realm_uuid}!");
                Ok(ClientReponse::Ok)
            }
            ClientCommand::UpdateApplication {
                realm_uuid,
                application_uuid,
                config: _,
            } => {
                info!("Starting application: {application_uuid} in realm: {realm_uuid}!");
                info!("Started application: {application_uuid} in realm: {realm_uuid}!");
                Ok(ClientReponse::Ok)
            }
        }
    }

    fn resolve_command(&self, serialized_command: String) -> Result<ClientCommand, ClientError> {
        let command: ClientCommand =
            serde_json::from_str(&serialized_command).map_err(|_| ClientError::UnknownCommand {
                size: serialized_command.len(),
            })?;
        Ok(command)
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
