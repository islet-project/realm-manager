use std::{io, path::PathBuf};

use thiserror::Error;
use utils::serde::json_framed::JsonFramedError;
use warden_client::warden::{WardenDaemonError, WardenResponse};

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
