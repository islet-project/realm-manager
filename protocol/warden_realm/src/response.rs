use serde::{Serialize, Deserialize};
use crate::error::ProtocolError;

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    ApplicationExited(i32),
    ApplicationIsRunning(),
    ApplicationNotStarted(),
    Success(),
    Error(ProtocolError)
}
