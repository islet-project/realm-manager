use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug, PartialEq, PartialOrd, Clone, Serialize, Deserialize)]
pub enum NetworkManagerError {
    #[error("Can't create a tap device: {tap_name} for realm: {err_message}")]
    TapCreation{
        tap_name: String,
        err_message: String,
    },
}

#[async_trait]
pub trait NetworkManager {
    async fn create_tap_device_for_realm(
        &self,
        name: &str,
    ) -> Result<(), NetworkManagerError>;
    async fn prepare_network(&self) -> Result<(), NetworkManagerError>;
}
