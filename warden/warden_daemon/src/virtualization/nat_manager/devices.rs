use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BridgeError {
    #[error("Failed to create bridge: {0}")]
    AddTap(String),
    #[error("Failed to delete bridge: {0}")]
    RemoveTap(String),
}

pub trait Tap {
    fn get_name(&self) -> &str;
}

#[async_trait]
pub trait Bridge {
    async fn add_tap_device_to_bridge(
        &mut self,
        tap: &(dyn Tap + Send + Sync),
    ) -> Result<(), BridgeError>;
    async fn remove_tap_device_from_bridge(
        &mut self,
        tap: &(dyn Tap + Send + Sync),
    ) -> Result<(), BridgeError>;
    fn get_name(&self) -> &str;
}
