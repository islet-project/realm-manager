use devicemapper::DmOptions;
use serde::Deserialize;
use std::fmt::Display;
use thiserror::Error;

use super::{
    device::{DeviceHandle, DeviceHandleWrapper},
    Result,
};

#[derive(Debug, Error)]
pub enum CryptError {}

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum Cipher {
    Aes,
    Twofish,
    Serpent,
}

impl Display for Cipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Cipher::Aes => write!(f, "aes"),
            Cipher::Twofish => write!(f, "twofish"),
            Cipher::Serpent => write!(f, "serpent"),
        }
    }
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum HashAlgo {
    Sha256,
}

impl Display for HashAlgo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HashAlgo::Sha256 => write!(f, "sha256"),
        }
    }
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum IvMode {
    Plain,
    Plain64,
    Essiv(HashAlgo),
}

impl Display for IvMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IvMode::Plain => write!(f, "plain"),
            IvMode::Plain64 => write!(f, "plain64"),
            IvMode::Essiv(h) => write!(f, "essiv:{}", h),
        }
    }
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum BlockMode {
    Cbc,
    Xts,
}

impl Display for BlockMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockMode::Cbc => write!(f, "cbc"),
            BlockMode::Xts => write!(f, "xts"),
        }
    }
}

#[allow(dead_code)]
pub enum KeyType {
    Logon,
    User,
    Encrypted,
}

impl Display for KeyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyType::User => write!(f, "user"),
            KeyType::Logon => write!(f, "logon"),
            KeyType::Encrypted => write!(f, "encrypted"),
        }
    }
}

#[allow(dead_code)]
pub enum Key {
    Raw(Vec<u8>),
    Hex(String),
    Keyring {
        key_size: usize,
        key_type: KeyType,
        key_desc: String,
    },
}

impl Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Key::Hex(h) => write!(f, "{}", h),
            Key::Raw(v) => write!(f, "{}", hex::encode(v)),
            Key::Keyring {
                key_size,
                key_type,
                key_desc,
            } => write!(f, ":{}:{}:{}", key_size, key_type, key_desc),
        }
    }
}

#[allow(dead_code)]
pub enum DevicePath {
    Name(String),
    MajorMinor(u32, u32),
}

impl Display for DevicePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DevicePath::Name(name) => write!(f, "/dev/{}", name),
            DevicePath::MajorMinor(major, minor) => write!(f, "{}:{}", major, minor),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct CryptoParams {
    pub cipher: Cipher,
    pub iv_mode: IvMode,
    pub block_mode: BlockMode,
    pub iv_offset: usize,
    pub additional_options: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct DmCryptTable<'a> {
    pub start: u64,
    pub len: u64,
    pub params: &'a CryptoParams,
    pub offset: u64,
}

pub struct CryptDevice(DeviceHandle);

impl CryptDevice {
    pub fn load(
        &self,
        entry: DmCryptTable,
        devpath: &DevicePath,
        key: &Key,
        options: Option<DmOptions>,
    ) -> Result<()> {
        let mut params = format!(
            "{}-{}-{} {} {} {} {}",
            entry.params.cipher,
            entry.params.block_mode,
            entry.params.iv_mode,
            key,
            entry.params.iv_offset,
            devpath,
            entry.offset
        );

        if let Some(opts) = &entry.params.additional_options {
            params.push_str(format!("{} {}", opts.len(), opts.join(" ")).as_str());
        }

        let table = vec![(entry.start, entry.len, "crypt".into(), params)];

        self.0.table_load(&table, options)?;

        Ok(())
    }
}

impl From<DeviceHandle> for CryptDevice {
    fn from(value: DeviceHandle) -> Self {
        Self(value)
    }
}

impl DeviceHandleWrapper for CryptDevice {
    fn handle(&self) -> &DeviceHandle {
        &self.0
    }
}
