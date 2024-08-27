use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug, PartialEq, PartialOrd, Clone, Serialize, Deserialize)]
pub enum NetworkManagerError {
    #[error("Can't create a tap device: {tap_name} for realm: {err_message}")]
    TapCreation {
        tap_name: String,
        err_message: String,
    },
    #[error("There is no tap for realm: {realm_uuid}.")]
    MissingTap { realm_uuid: Uuid },
    #[error("Failed to add interface: {tap_name} to the bridge: {bridge_name}")]
    BridgeAddIf {
        tap_name: String,
        bridge_name: String,
    },
    #[error("Failed to delete interface: {tap_name} from the bridge: {bridge_name}")]
    BridgeDelIf {
        tap_name: String,
        bridge_name: String,
    },
    #[error("Failed to perform bridge operation: {0}")]
    BridgeOperation(String),
}

#[async_trait]
pub trait NetworkManager {
    async fn create_tap_device_for_realm(
        &mut self,
        name: String,
        realm_id: Uuid,
    ) -> Result<(), NetworkManagerError>;
    async fn prepare_network(&self) -> Result<(), NetworkManagerError>;
    async fn shutdown_tap_device_for_realm(
        &mut self,
        realm_id: Uuid,
    ) -> Result<(), NetworkManagerError>;
}
