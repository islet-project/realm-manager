use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::realm::RealmDescription;
use crate::{applciation::ApplicationConfig, realm::RealmConfig};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum WardenCommand {
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

#[derive(Debug, Error, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum WardenDaemonError {
    #[error("Failed to read request.")]
    ReadingRequestFail,
    #[error("Can't recognise a command.")]
    UnknownCommand,
    #[error("Error occured: {message}")]
    WardenDaemonFail { message: String },
    #[error("Failed to send response.")]
    SendingResponseFail,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum WardenResponse {
    Ok,
    CreatedRealm {
        uuid: Uuid,
    },
    CreatedApplication {
        uuid: Uuid,
    },
    InspectedRealm {
        description: RealmDescription,
    },
    ListedRealms {
        realms_description: Vec<RealmDescription>,
    },
    Error {
        warden_error: WardenDaemonError,
    },
}
