use async_trait::async_trait;
use log::info;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;
use tokio::{net::UnixStream, select};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::managers::realm::RealmError;
use crate::managers::realm_configuration::RealmConfig;

use crate::managers::warden::{Warden, WardenError};

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientCommand {
    CreateRealm { config: RealmConfig },
    StartRealm { uuid: String },
    StopRealm { uuid: String },
    DestroyRealm { uuid: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientReponse {
    CreatedRealm { uuid: String },
}

#[derive(Debug)]
pub enum ClientError {
    ReadingRequestFail,
    UnknownCommand,
    RealmCreationFail,
    InvalidResponse,
    SendingResponseFail,
    InvalidUuid,
    HostDaemonError(WardenError),
    RealmManagerError(RealmError),
}

#[async_trait]
pub trait Client {
    async fn handle_connection(
        host: Arc<Mutex<Box<dyn Warden + Send + Sync>>>,
        socket: UnixStream,
        token: Arc<CancellationToken>,
    ) -> Result<(), ClientError>;
}

pub struct ClientHandler {
    host: Arc<Mutex<Box<dyn Warden + Send + Sync>>>,
    socket: BufReader<UnixStream>,
    token: Arc<CancellationToken>,
}

impl ClientHandler {
    pub async fn handle_requests(&mut self) -> Result<(), ClientError> {
        // TODO! Handle socket disconnection!
        loop {
            let mut request_data = String::new();
            select! {
                readed_bytes = self.socket.read_line(&mut request_data) => {
                    info!("Received message: {request_data}");
                    let _ = readed_bytes.map_err(|_|ClientError::ReadingRequestFail)?;
                    let command = self.resolve_command(request_data)?;
                    let res = self.handle_command(command).await?;
                }
                _ = self.token.cancelled() => {
                    break;
                }
            }
        }
        Ok(())
    }

    async fn send_response(&mut self, response: ClientReponse) -> Result<(), ClientError> {
        todo!()
    }

    async fn handle_command(&mut self, client_command: ClientCommand) -> Result<(), ClientError> {
        match client_command {
            ClientCommand::StartRealm { uuid } => {
                info!("Starting realm: {uuid}");
                let _ = self
                    .host
                    .lock()
                    .await
                    .as_mut()
                    .get_realm(
                        &Uuid::from_str(&uuid).map_err(|_| ClientError::InvalidUuid)?,
                    )
                    .map_err(|err| ClientError::HostDaemonError(err))?
                    .start()
                    .await
                    .map_err(|err| ClientError::RealmManagerError(err))?;
                info!("Started realm: {uuid}");
                Ok(())
            }
            ClientCommand::CreateRealm { config } => {
                info!("Creating realm!");
                let uuid = self
                    .host
                    .lock()
                    .await
                    .create_realm(config)
                    .map_err(|err| ClientError::HostDaemonError(err))?;
                info!("Created realm: {uuid}");
                Ok(())
            }
            ClientCommand::StopRealm { uuid } => {
                info!("Stopping realm: {uuid}");
                let _ = self
                    .host
                    .lock()
                    .await
                    .get_realm(
                        &Uuid::from_str(&uuid).map_err(|_| ClientError::InvalidUuid)?,
                    )
                    .map_err(|err| ClientError::HostDaemonError(err))?
                    .stop();
                info!("Stopped realm: {uuid}");
                Ok(())
            }
            ClientCommand::DestroyRealm { uuid } => {
                info!("Destroying realm: {uuid}");
                let _ =
                    self.host.lock().await.destroy_realm(
                        Uuid::from_str(&uuid).map_err(|_| ClientError::InvalidUuid)?,
                    );
                info!("Destroyed realm: {uuid}");
                Ok(())
            }
        }
    }

    fn resolve_command(&self, serialized_command: String) -> Result<ClientCommand, ClientError> {
        let command: ClientCommand =
            serde_json::from_str(&serialized_command).map_err(|_| ClientError::UnknownCommand)?;
        Ok(command)
    }
}

#[async_trait]
impl Client for ClientHandler {
    async fn handle_connection(
        host: Arc<Mutex<Box<dyn Warden + Send + Sync>>>,
        socket: UnixStream,
        token: Arc<CancellationToken>,
    ) -> Result<(), ClientError> {
        let mut handler = ClientHandler {
            host,
            socket: BufReader::new(socket),
            token,
        };
        handler.handle_requests().await
    }
}
