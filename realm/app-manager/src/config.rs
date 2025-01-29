use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::Result;
use rust_rsi::{
    RSI_SEALING_KEY_FLAGS_KEY, RSI_SEALING_KEY_FLAGS_REALM_ID, RSI_SEALING_KEY_FLAGS_RIM,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::dm::crypt::CryptoParams;
use crate::util::fs::{read_to_string, Filesystem};

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Deserialization error")]
    InvalidConfigFile(#[from] serde_yaml::Error),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TokenResolver {
    #[serde(rename = "file")]
    File(String),

    #[serde(rename = "rsi")]
    Rsi,
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

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum RsiSealingKeyFlags {
    Key,
    Rim,
    RealmId,
}

impl From<&RsiSealingKeyFlags> for u64 {
    fn from(value: &RsiSealingKeyFlags) -> Self {
        match value {
            RsiSealingKeyFlags::Key => RSI_SEALING_KEY_FLAGS_KEY,
            RsiSealingKeyFlags::Rim => RSI_SEALING_KEY_FLAGS_RIM,
            RsiSealingKeyFlags::RealmId => RSI_SEALING_KEY_FLAGS_REALM_ID,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IkmSource {
    StubbedHex(String),

    RsiSealingKey {
        flags: HashSet<RsiSealingKeyFlags>,
        svn: Option<u64>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum KeySealingType {
    Dummy,
    HkdfSha256(IkmSource),
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum ExtendREM {
    #[serde(rename = "rem0")]
    Rem0 = 1,

    #[serde(rename = "rem1")]
    Rem1 = 2,

    #[serde(rename = "rem2")]
    Rem2 = 3,

    #[serde(rename = "rem3")]
    Rem3 = 4,
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

    pub ca_pub: PathBuf,

    #[serde(with = "serde_yaml::with::singleton_map_recursive")]
    pub keysealing: KeySealingType,

    pub autostartall: bool,
    pub extend: Option<ExtendREM>,

    pub ensure_same_app_vendor: bool,
    pub fs_type: Filesystem
}

impl Config {
    pub async fn read_from_file(path: impl AsRef<Path>) -> Result<Config> {
        let content = read_to_string(path).await?;

        Ok(serde_yaml::from_str(&content).map_err(ConfigError::InvalidConfigFile)?)
    }

    pub fn requires_rsi(&self) -> bool {
        matches!(
            (&self.keysealing, &self.launcher, &self.extend),
            (
                KeySealingType::HkdfSha256(IkmSource::RsiSealingKey { .. }),
                _,
                _
            ) | (
                _,
                LauncherType::Oci(OciLauncherConfig::RaTLS {
                    token_resolver: TokenResolver::Rsi,
                    ..
                }),
                _
            ) | (_, _, Some(_))
        )
    }
}
