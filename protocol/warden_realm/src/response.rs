use serde::{Serialize, Deserialize};
use crate::error::ProtocolError;

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    ProvisioningFinished(),
    ApplicationExited(i32),
    ApplicationIsRunning(),
    Success(),
    Error(ProtocolError)
}
