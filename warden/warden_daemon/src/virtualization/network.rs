use std::net::IpAddr;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug, PartialEq, PartialOrd, Clone, Serialize, Deserialize)]
pub enum NetworkManagerError {
    #[error("Failed to create nat network: {0}.")]
    CreateNatNetwork(String),
    #[error("Failed to destroy nat network: {0}.")]
    DestroyNatNetwork(String),
    #[error("Failed to create tap device: {0}")]
    CreateTapDevice(String),
    #[error("Failed to destroy tap device: {0}")]
    DestroyTapDevice(String),
}

#[derive(Debug, Clone)]
pub struct NatConfig {
    pub net_if_name: String,
    pub net_if_ip: IpAddr,
    pub net_if_mask: u8,
}

#[async_trait]
pub trait NetworkManager {
    async fn create_nat(config: NatConfig) -> Result<Self, NetworkManagerError>
    where
        Self: Sized;

    async fn create_tap_device_for_realm(
        &mut self,
        name: String,
        realm_id: Uuid,
    ) -> Result<(), NetworkManagerError>;
    async fn shutdown_tap_device_for_realm(
        &mut self,
        realm_id: Uuid,
    ) -> Result<(), NetworkManagerError>;

    async fn shutdown_nat(&mut self) -> Result<(), NetworkManagerError>;
}
