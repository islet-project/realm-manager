use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum ProtocolError {
    InvalidRequest(String),
    ProvisionInfoNotReceived(),
    ApplicationsAlreadyProvisioned(),
    ApplicationNotFound(),
    ApplicationLaunchFailed(String),
    ApplicationStopFailed(String),
    ApplicationKillFailed(String),
    ApplicationCheckStatusFailed(String),
    SystemPowerActionFailed(String),
    GetIfAddrsError(String),
}
