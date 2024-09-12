use std::path::{Path, PathBuf};

use super::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::dm::crypt::CryptoParams;
use crate::util::fs::read_to_string;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Deserialization error")]
    InvalidConfigFile(#[from] serde_yaml::Error),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TokenResolver {
    #[serde(rename = "from_file")]
    FromFile(String),
    // TODO: Add RSI
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum OciLauncherConfig {
    NoTLS,

    RusTLS {
        root_ca: String,
    },

    RaTLS {
        root_ca: String,
        token_resolver: TokenResolver,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum LauncherType {
    Dummy,
    Oci(OciLauncherConfig),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum KeySealingType {
    Dummy,
}

#[allow(dead_code)]
#[non_exhaustive]
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub workdir: PathBuf,
    pub vsock_port: u32,
    pub crypto: CryptoParams,

    #[serde(with = "serde_yaml::with::singleton_map_recursive")]
    pub launcher: LauncherType,

    #[serde(with = "serde_yaml::with::singleton_map_recursive")]
    pub keysealing: KeySealingType,

    pub autostartall: bool,
}

impl Config {
    pub async fn read_from_file(path: impl AsRef<Path>) -> Result<Config> {
        let content = read_to_string(path).await?;

        Ok(serde_yaml::from_str(&content).map_err(ConfigError::InvalidConfigFile)?)
    }
}
