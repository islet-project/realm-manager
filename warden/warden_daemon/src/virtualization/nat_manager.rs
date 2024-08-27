use std::collections::HashSet;

use async_trait::async_trait;
use tokio_tun::TunBuilder;

use super::network_manager::{NetworkManager, NetworkManagerError};

pub struct NatManager {
    taps: HashSet<String>
}

impl NatManager {
    pub fn new() -> Self {
        Self {taps: HashSet::new()}
    }
}

#[async_trait]
impl NetworkManager for NatManager {
    async fn create_tap_device_for_realm(
        &self,
        name: &str,
    ) -> Result<(), NetworkManagerError> {
        if self.taps.contains(name) {
            return Err(NetworkManagerError::TapCreation { tap_name: name.to_string(), err_message: "already exists.".to_string() })
        }
        TunBuilder::new()
            .name(name)
            .tap(true)
            .persist()
            .try_build()
            .map_err(|err| NetworkManagerError::TapCreation { tap_name: name.to_string(), err_message: err.to_string() })?;
        Ok(())
    }
    async fn prepare_network(&self) -> Result<(), NetworkManagerError> {
        Ok(())
    }
}
