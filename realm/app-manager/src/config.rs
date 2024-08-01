use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;
use super::Result;
use crate::util::fs::read_to_string;
use crate::dm::crypt::CryptoParams;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Deserialization error")]
    InvalidConfigFile(#[from] serde_yaml::Error)
}

#[derive(Debug, Deserialize)]
pub enum LauncherType {
    Dummy
}

#[derive(Debug, Deserialize)]
pub enum KeySealingType {
    Dummy
}

#[allow(dead_code)]
#[non_exhaustive]
#[derive(Debug, Deserialize)]
pub struct Config {
    pub workdir: PathBuf,
    pub vsock_port: u32,
    pub crypto: CryptoParams,
    pub image_registry: String,
    pub launcher: LauncherType,
    pub keysealing: KeySealingType,
    pub autostartall: bool
}

impl Config {
    pub async fn read_from_file(path: impl AsRef<Path>) -> Result<Config> {
        let content = read_to_string(path).await?;

        Ok(
            serde_yaml::from_str(&content)
                .map_err(ConfigError::InvalidConfigFile)?
        )
    }
}
