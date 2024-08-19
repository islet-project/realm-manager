use thiserror::Error;

use crate::app::ApplicationError;
use crate::dm::DeviceMapperError;
use crate::key::KeyError;
use crate::launcher::LauncherError;
use crate::util::UtilsError;
use crate::config::ConfigError;
use crate::manager::ManagerError;

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
    UtilError(#[from] UtilsError),
}

pub type Result<T> = std::result::Result<T, Error>;

