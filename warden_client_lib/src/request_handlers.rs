use std::path::PathBuf;

use tokio::net::UnixStream;
use utils::serde::JsonFramed;
use uuid::Uuid;
use warden_client::{
    applciation::ApplicationConfig,
    warden::{WardenCommand, WardenResponse},
    realm::{RealmConfig, RealmDescription},
};

use crate::warden_client_error::WardenClientError;

type Communicator = JsonFramed<UnixStream, WardenResponse, WardenCommand>;

pub async fn create_realm(
    communicator: &mut Communicator,
    config: RealmConfig,
) -> Result<Uuid, WardenClientError> {
    match communicate(communicator, WardenCommand::CreateRealm { config }).await? {
        WardenResponse::CreatedRealm { uuid } => Ok(uuid),
        response => Err(handle_error_response(response)),
    }
}

pub async fn start_realm(
    communicator: &mut Communicator,
    uuid: Uuid,
) -> Result<(), WardenClientError> {
    match communicate(communicator, WardenCommand::StartRealm { uuid }).await? {
        WardenResponse::Ok => Ok(()),
        response => Err(handle_error_response(response)),
    }
}

pub async fn stop_realm(
    communicator: &mut Communicator,
    uuid: Uuid,
) -> Result<(), WardenClientError> {
    match communicate(communicator, WardenCommand::StopRealm { uuid }).await? {
        WardenResponse::Ok => Ok(()),
        response => Err(handle_error_response(response)),
    }
}

pub async fn reboot_realm(
    communicator: &mut Communicator,
    uuid: Uuid,
) -> Result<(), WardenClientError> {
    match communicate(communicator, WardenCommand::RebootRealm { uuid }).await? {
        WardenResponse::Ok => Ok(()),
        response => Err(handle_error_response(response)),
    }
}

pub async fn destroy_realm(
    communicator: &mut Communicator,
    uuid: Uuid,
) -> Result<(), WardenClientError> {
    match communicate(communicator, WardenCommand::DestroyRealm { uuid }).await? {
        WardenResponse::Ok => Ok(()),
        response => Err(handle_error_response(response)),
    }
}

pub async fn inspect_realm(
    communicator: &mut Communicator,
    uuid: Uuid,
) -> Result<RealmDescription, WardenClientError> {
    match communicate(communicator, WardenCommand::InspectRealm { uuid }).await? {
        WardenResponse::InspectedRealm { description } => Ok(description),
        response => Err(handle_error_response(response)),
    }
}

pub async fn list_realms(
    communicator: &mut Communicator,
) -> Result<Vec<RealmDescription>, WardenClientError> {
    match communicate(communicator, WardenCommand::ListRealms).await? {
        WardenResponse::ListedRealms { realms_description } => Ok(realms_description),
        response => Err(handle_error_response(response)),
    }
}

pub async fn create_application(
    communicator: &mut Communicator,
    realm_uuid: Uuid,
    config: ApplicationConfig,
) -> Result<Uuid, WardenClientError> {
    match communicate(
        communicator,
        WardenCommand::CreateApplication { realm_uuid, config },
    )
    .await?
    {
        WardenResponse::CreatedApplication { uuid } => Ok(uuid),
        response => Err(handle_error_response(response)),
    }
}

pub async fn update_application(
    communicator: &mut Communicator,
    realm_uuid: Uuid,
    application_uuid: Uuid,
    config: ApplicationConfig,
) -> Result<(), WardenClientError> {
    match communicate(
        communicator,
        WardenCommand::UpdateApplication {
            realm_uuid,
            application_uuid,
            config,
        },
    )
    .await?
    {
        WardenResponse::Ok => Ok(()),
        response => Err(handle_error_response(response)),
    }
}

pub async fn start_application(
    communicator: &mut Communicator,
    realm_uuid: Uuid,
    application_uuid: Uuid,
) -> Result<(), WardenClientError> {
    match communicate(
        communicator,
        WardenCommand::StartApplication {
            realm_uuid,
            application_uuid,
        },
    )
    .await?
    {
        WardenResponse::Ok => Ok(()),
        response => Err(handle_error_response(response)),
    }
}

pub async fn stop_application(
    communicator: &mut Communicator,
    realm_uuid: Uuid,
    application_uuid: Uuid,
) -> Result<(), WardenClientError> {
    match communicate(
        communicator,
        WardenCommand::StopApplication {
            realm_uuid,
            application_uuid,
        },
    )
    .await?
    {
        WardenResponse::Ok => Ok(()),
        response => Err(handle_error_response(response)),
    }
}

pub async fn connect_to_warden_sokcet(
    warden_socket_path: PathBuf,
) -> Result<UnixStream, WardenClientError> {
    UnixStream::connect(&warden_socket_path)
        .await
        .map_err(|err| WardenClientError::ConnectionFailed {
            socket_path: warden_socket_path,
            details: err,
        })
}

async fn communicate(
    communicator: &mut Communicator,
    command: WardenCommand,
) -> Result<WardenResponse, WardenClientError> {
    communicator
        .send(command)
        .await
        .map_err(WardenClientError::CommunicationFail)?;
    communicator
        .recv()
        .await
        .map_err(WardenClientError::CommunicationFail)
}

fn handle_error_response(response: WardenResponse) -> WardenClientError {
    match response {
        WardenResponse::Error { warden_error } => {
            WardenClientError::WardenOperationFail(warden_error)
        }
        response => WardenClientError::InvalidResponse { response },
    }
}
