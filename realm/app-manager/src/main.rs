use app::ApplicationError;
use config::{Config, ConfigError};
use dm::DeviceMapperError;
use key::KeyError;
use manager::{Manager, ManagerError};
use thiserror::Error;
use util::UtilsError;
use log::info;

mod app;
mod config;
mod dm;
mod key;
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
    let _ = manager.setup().await?;

    info!("Applications started entering event loop");
    let _ = manager.handle_events().await?;

    Ok(())
}
