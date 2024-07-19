use std::{ffi::OsStr, num::ParseIntError, path::{Path, PathBuf}};

use thiserror::Error;
use tokio::{fs::{self, File}, io::AsyncReadExt};
use super::Result;
use std::str::FromStr;


#[derive(Debug, Error)]
pub enum DiskError {
    #[error("Error resolving symlink")]
    ReadLinkError(#[source] std::io::Error),

    #[error("Empty device name")]
    EmptyDeviceName(),

    #[error("Device could not be found in sysfs")]
    DeviceNotFound(#[source] std::io::Error),

    #[error("Error reading size from sysfs")]
    SysfsFileReadError(#[source] std::io::Error),

    #[error("Disk size is not an integer")]
    InvalidDiskSize(#[from] ParseIntError)
}

pub async fn read_device_size(path: impl AsRef<Path>) -> Result<u64> {
    let realpath = if path.as_ref().is_symlink() {
        fs::read_link(path).await.map_err(DiskError::ReadLinkError)?
    } else {
        path.as_ref().to_owned()
    };

    let device_name = realpath.file_name().ok_or(DiskError::EmptyDeviceName())?;
    let sysfs_path: PathBuf = [ OsStr::new("/sys/class/block/"), device_name, OsStr::new("size") ].iter().collect();

    let mut file = File::open(sysfs_path).await.map_err(DiskError::DeviceNotFound)?;
    let mut content = String::new();
    file.read_to_string(&mut content).await.map_err(DiskError::SysfsFileReadError)?;

    Ok(u64::from_str(content.trim()).map_err(DiskError::InvalidDiskSize)?)
}
