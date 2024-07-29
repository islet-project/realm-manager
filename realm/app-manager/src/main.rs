use std::{path::Path, str::FromStr};

use app::{ApplicationError, ApplicationInfo};
use config::{Config, ConfigError};
use dm::DeviceMapperError;
use key::KeyError;
use launcher::{dummy::DummyLauncher, Launcher, LauncherError};
use manager::{Manager, ManagerError};
use thiserror::Error;
use util::UtilsError;
use log::info;
use uuid::Uuid;

mod app;
mod config;
mod dm;
mod key;
mod launcher;
mod manager;
mod util;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Application error")]
    ApplicationError(#[from] ApplicationError),

    #[error("Config error")]
    ConfigError(#[from] ConfigError),

    #[error("Key error")]
    KeyError(#[from] KeyError),

    #[error("Launcher error")]
    LauncherError(#[from] LauncherError),

    #[error("Device mapper error")]
    DMError(#[from] DeviceMapperError),

    #[error("Manager error")]
    ManagerError(#[from] ManagerError),

    #[error("Utilities error")]
    UtilError(#[from] UtilsError)
}

pub type Result<T> = std::result::Result<T, Error>;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let config = Config::read_from_file("/etc/app-manager/config.yml").await?;
    let mut manager = Manager::new(config)?;

    info!("Provishioning...");
    let _ = manager.setup(Path::new("/apps"), vec![
        ApplicationInfo {
            id: Uuid::new_v4(),
            name: "Testapp".to_owned(),
            version: "1.1.1".to_owned(),
            image_part_uuid: Uuid::from_str("2fd89730-d156-6548-baf3-13b3040b2efb").unwrap(),
            data_part_uuid: Uuid::from_str("74b3a3d5-2218-ff47-9aa2-d3fd4edb347f").unwrap()
        }
    ]).await?;

    info!("Applications started entering event loop");
    let _ = manager.handle_events().await?;

    Ok(())
}
