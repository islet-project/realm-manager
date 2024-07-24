use std::{io, path::PathBuf};
use thiserror::Error;
use tokio::net::UnixStream;
use utils::serde::{JsonFramed, JsonFramedError};
use uuid::Uuid;
use warden_client::{
    applciation::ApplicationConfig,
    client::{WardenCommand, WardenDaemonError, WardenResponse},
    realm::{RealmConfig, RealmDescription},
};

#[derive(Debug, Error)]
pub enum WardenClientError {
    #[error(
        "Failed to connect to Warden's socket at path: {socket_path}. More details: {details}"
    )]
    ConnectionFailed {
        socket_path: PathBuf,
        #[source]
        details: io::Error,
    },
    #[error("Warden operation failed: {0}")]
    WardenOperationFail(#[from] WardenDaemonError),
    #[error("Failed to communicate with Warden: {0}")]
    CommunicationFail(#[from] JsonFramedError),
    #[error("Invalid response: {response:#?}")]
    InvalidResponse { response: WardenResponse },
}

pub struct WardenConnection {
    communicator: JsonFramed<UnixStream, WardenResponse, WardenCommand>,
}

impl WardenConnection {
    pub async fn connect(
        warden_socket_path: PathBuf,
    ) -> Result<WardenConnection, WardenClientError> {
        let stream = UnixStream::connect(&warden_socket_path)
            .await
            .map_err(|err| WardenClientError::ConnectionFailed {
                socket_path: warden_socket_path,
                details: err,
            })?;
        Ok(WardenConnection {
            communicator: JsonFramed::<UnixStream, WardenResponse, WardenCommand>::new(stream),
        })
    }

    pub async fn create_realm(&mut self, config: RealmConfig) -> Result<Uuid, WardenClientError> {
        self.communicator
            .send(WardenCommand::CreateRealm { config })
            .await
            .map_err(WardenClientError::CommunicationFail)?;
        match self
            .communicator
            .recv()
            .await
            .map_err(WardenClientError::CommunicationFail)?
        {
            WardenResponse::CreatedRealm { uuid } => Ok(uuid),
            response => Err(self.handle_error_response(response)),
        }
    }

    pub async fn start_realm(&mut self, uuid: Uuid) -> Result<(), WardenClientError> {
        self.communicator
            .send(WardenCommand::StartRealm { uuid })
            .await
            .map_err(WardenClientError::CommunicationFail)?;
        match self
            .communicator
            .recv()
            .await
            .map_err(WardenClientError::CommunicationFail)?
        {
            WardenResponse::Ok => Ok(()),
            response => Err(self.handle_error_response(response)),
        }
    }

    pub async fn stop_realm(&mut self, uuid: Uuid) -> Result<(), WardenClientError> {
        self.communicator
            .send(WardenCommand::StopRealm { uuid })
            .await
            .map_err(WardenClientError::CommunicationFail)?;
        match self
            .communicator
            .recv()
            .await
            .map_err(WardenClientError::CommunicationFail)?
        {
            WardenResponse::Ok => Ok(()),
            response => Err(self.handle_error_response(response)),
        }
    }

    pub async fn reboot_realm(&mut self, uuid: Uuid) -> Result<(), WardenClientError> {
        self.communicator
            .send(WardenCommand::RebootRealm { uuid })
            .await
            .map_err(WardenClientError::CommunicationFail)?;
        match self
            .communicator
            .recv()
            .await
            .map_err(WardenClientError::CommunicationFail)?
        {
            WardenResponse::Ok => Ok(()),
            response => Err(self.handle_error_response(response)),
        }
    }

    pub async fn destroy_realm(&mut self, uuid: Uuid) -> Result<(), WardenClientError> {
        self.communicator
            .send(WardenCommand::DestroyRealm { uuid })
            .await
            .map_err(WardenClientError::CommunicationFail)?;
        match self
            .communicator
            .recv()
            .await
            .map_err(WardenClientError::CommunicationFail)?
        {
            WardenResponse::Ok => Ok(()),
            response => Err(self.handle_error_response(response)),
        }
    }

    pub async fn inspect_realm(
        &mut self,
        uuid: Uuid,
    ) -> Result<RealmDescription, WardenClientError> {
        self.communicator
            .send(WardenCommand::InspectRealm { uuid })
            .await
            .map_err(WardenClientError::CommunicationFail)?;
        match self
            .communicator
            .recv()
            .await
            .map_err(WardenClientError::CommunicationFail)?
        {
            WardenResponse::InspectedRealm { description } => Ok(description),
            response => Err(self.handle_error_response(response)),
        }
    }

    pub async fn list_realms(&mut self) -> Result<Vec<RealmDescription>, WardenClientError> {
        self.communicator
            .send(WardenCommand::ListRealms)
            .await
            .map_err(WardenClientError::CommunicationFail)?;
        match self
            .communicator
            .recv()
            .await
            .map_err(WardenClientError::CommunicationFail)?
        {
            WardenResponse::ListedRealms { realms_description } => Ok(realms_description),
            response => Err(self.handle_error_response(response)),
        }
    }

    pub async fn create_application(
        &mut self,
        realm_uuid: Uuid,
        config: ApplicationConfig,
    ) -> Result<Uuid, WardenClientError> {
        self.communicator
            .send(WardenCommand::CreateApplication { realm_uuid, config })
            .await
            .map_err(WardenClientError::CommunicationFail)?;
        match self
            .communicator
            .recv()
            .await
            .map_err(WardenClientError::CommunicationFail)?
        {
            WardenResponse::CreatedApplication { uuid } => Ok(uuid),
            response => Err(self.handle_error_response(response)),
        }
    }

    pub async fn update_application(
        &mut self,
        realm_uuid: Uuid,
        application_uuid: Uuid,
        config: ApplicationConfig,
    ) -> Result<(), WardenClientError> {
        self.communicator
            .send(WardenCommand::UpdateApplication {
                realm_uuid,
                application_uuid,
                config,
            })
            .await
            .map_err(WardenClientError::CommunicationFail)?;
        match self
            .communicator
            .recv()
            .await
            .map_err(WardenClientError::CommunicationFail)?
        {
            WardenResponse::Ok => Ok(()),
            response => Err(self.handle_error_response(response)),
        }
    }

    pub async fn start_application(
        &mut self,
        realm_uuid: Uuid,
        application_uuid: Uuid,
    ) -> Result<(), WardenClientError> {
        self.communicator
            .send(WardenCommand::StartApplication {
                realm_uuid,
                application_uuid,
            })
            .await
            .map_err(WardenClientError::CommunicationFail)?;
        match self
            .communicator
            .recv()
            .await
            .map_err(WardenClientError::CommunicationFail)?
        {
            WardenResponse::Ok => Ok(()),
            response => Err(self.handle_error_response(response)),
        }
    }

    pub async fn stop_application(
        &mut self,
        realm_uuid: Uuid,
        application_uuid: Uuid,
    ) -> Result<(), WardenClientError> {
        self.communicator
            .send(WardenCommand::StopApplication {
                realm_uuid,
                application_uuid,
            })
            .await
            .map_err(WardenClientError::CommunicationFail)?;
        match self
            .communicator
            .recv()
            .await
            .map_err(WardenClientError::CommunicationFail)?
        {
            WardenResponse::Ok => Ok(()),
            response => Err(self.handle_error_response(response)),
        }
    }

    fn handle_error_response(&self, response: WardenResponse) -> WardenClientError {
        match response {
            WardenResponse::Error { warden_error } => {
                WardenClientError::WardenOperationFail(warden_error)
            }
            response => WardenClientError::InvalidResponse { response },
        }
    }
}
