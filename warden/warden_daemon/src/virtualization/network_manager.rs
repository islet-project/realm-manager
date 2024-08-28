use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug, PartialEq, PartialOrd, Clone, Serialize, Deserialize)]
pub enum NetworkManagerError {
    #[error("Can't create a bride: {bridge_name} err: {err_message}")]
    BridgeCreation {
        bridge_name: String,
        err_message: String,
    },
    #[error("Can't assign an ip to a bride: {bridge_name} err: {err_message}")]
    BridgeIpAssign {
        bridge_name: String,
        err_message: String,
    },
    #[error("Can't set up bride: {bridge_name} err: {err_message}")]
    BridgeUp {
        bridge_name: String,
        err_message: String,
    },
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
    #[error("There is no tap for realm: {realm_uuid}.")]
    MissingTap { realm_uuid: Uuid },
    #[error("Can't create a tap device: {tap_name} for realm: {err_message}")]
    TapCreation {
        tap_name: String,
        err_message: String,
    },
    #[error("Failed to perform rtnetlink operation: {0}")]
    NetLinkOperation(String),
    #[error("Failed to perform iptables operation: {0}")]
    IpTablesOperation(String),
    #[error("Missing device: {0}")]
    MissingDevice(String),
}

#[async_trait]
pub trait NetworkManager {
    async fn create_tap_device_for_realm(
        &mut self,
        name: String,
        realm_id: Uuid,
    ) -> Result<(), NetworkManagerError>;
    async fn shutdown_tap_device_for_realm(
        &mut self,
        realm_id: Uuid,
    ) -> Result<(), NetworkManagerError>;
}
