use std::sync::Arc;

use thiserror::Error;

use crate::{config::Config, dm::DeviceMapper, key::KernelKeyring};

use super::Result;

#[derive(Debug, Error)]
pub enum ManagerError {

}

pub struct Manager {
    config: Config,
    devicemapper: Arc<DeviceMapper>,
    keyring: KernelKeyring,
}

impl Manager {
    pub fn new(config: Config) -> Result<Self> {
        let devicemapper = Arc::new(DeviceMapper::init()?);
        let keyring = KernelKeyring::new(keyutils::SpecialKeyring::User)?;

        Ok(Self { config, devicemapper, keyring })
    }

    pub async fn setup(&mut self) -> Result<()> {
        todo!()
    }

    pub async fn handle_events(&mut self) -> Result<()> {
        todo!()
    }
}
