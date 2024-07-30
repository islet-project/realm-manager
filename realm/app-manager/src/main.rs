use std::{path::Path, str::FromStr};
use app::ApplicationError;
use clap::Parser;
use cli::Args;
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
mod cli;
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
    let args = Args::parse();
    simple_logger::init_with_level(args.log_level).unwrap();

    info!("Reading config file: {:?}", args.config);
    let config = Config::read_from_file(args.config).await?;
    let mut manager = Manager::new(config).await?;

    info!("Provishioning...");
    let _ = manager.setup().await?;

    info!("Applications started entering event loop");
    let _ = manager.handle_events().await?;

    Ok(())
}
