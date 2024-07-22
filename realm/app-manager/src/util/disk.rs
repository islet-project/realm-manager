use std::{ffi::OsStr, num::ParseIntError, path::{Path, PathBuf}};

use thiserror::Error;
use tokio::{fs::{self, File}, io::AsyncReadExt};
use super::{fs::{read_to_string, readlink}, Result};
use std::str::FromStr;


#[derive(Debug, Error)]
pub enum DiskError {
    #[error("Empty device name")]
    EmptyDeviceName(),

    #[error("Disk size is not an integer")]
    InvalidDiskSize(#[from] ParseIntError)
}

pub async fn read_device_size(path: impl AsRef<Path>) -> Result<u64> {
    let realpath = if path.as_ref().is_symlink() {
        readlink(path).await?
    } else {
        path.as_ref().to_owned()
    };

    let device_name = realpath.file_name().ok_or(DiskError::EmptyDeviceName())?;
    let sysfs_path: PathBuf = [ OsStr::new("/sys/class/block/"), device_name, OsStr::new("size") ].iter().collect();
    let content = read_to_string(sysfs_path).await?;

    Ok(u64::from_str(content.trim()).map_err(DiskError::InvalidDiskSize)?)
}
