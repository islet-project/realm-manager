use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum ProtocolError {
    InvalidRequest(String),
    ApplicationNotFound(),
    ApplicationLaunchFailed(String),
    ApplicationStopFailed(String),
    ApplicationKillFailed(String),
    ApplicationCheckStatusFailed(String),
    SystemPowerActionFailed(String),
    GetIfAddrsError(String),
    ProvisioningError(String),
    AttestationTokenReadingError(String)
}
