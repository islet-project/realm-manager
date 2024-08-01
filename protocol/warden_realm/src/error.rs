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
    ApplicationWaitFailed(String),
    RebootActionFailed(String)
}
