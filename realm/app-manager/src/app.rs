use std::{os::linux::fs::MetadataExt, path::{Path, PathBuf}, sync::Arc};

use nix::libc::{major, makedev, minor};
use thiserror::Error;
use tokio::fs;
use uuid::Uuid;

use crate::{dm::{crypt::{CryptDevice, CryptoParams, DmCryptTable, Key}, device::{DeviceHandleWrapper, DeviceHandleWrapperExt}, DeviceMapper}, util::{disk::read_device_size, fs::{format, mkdirp, mknod, mount, readlink, stat, Filesystem}}};

use super::Result;

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("Partition not found")]
    PartitionNotFound(),
}

pub struct Application {
    workdir: PathBuf,
    devicemapper: Arc<DeviceMapper>
}

enum MountOption<'a> {
    JustMount,

    Format {
        label: Option<&'a str>
    }
}

impl Application {
    pub fn new(workdir: impl ToOwned<Owned = PathBuf>, devicemapper: &Arc<DeviceMapper>) -> Self {
        Self {
            workdir: workdir.to_owned(),
            devicemapper: Arc::clone(&devicemapper)
        }
    }

    async fn decrypt_partition(&self, part_uuid: &Uuid, params: &CryptoParams, key: Key, dst: impl AsRef<Path>) -> Result<CryptDevice> {
        let uuid_str = part_uuid.to_string();
        let path = Path::new("/dev/disk/by-partuuid/").join(&uuid_str);

        if !path.is_symlink() {
            return Err(ApplicationError::PartitionNotFound().into())
        }

        let metadata = stat(&path).await?;
        let major = unsafe { major(metadata.st_rdev()) };
        let minor = unsafe { minor(metadata.st_rdev()) };
        let size = read_device_size(&path).await?;
        let device_name = format!("crypt_{}", &uuid_str);

        let table = DmCryptTable {
            start: 0,
            len: size,
            params,
            offset: 0
        };

        let device: CryptDevice = self.devicemapper.create_device(device_name, None::<Uuid>, None)?;
        let _ = device.load(
            table,
            &crate::dm::crypt::DevicePath::MajorMinor(major, minor),
            &key,
            None
        )?;
        let _ = device.resume()?;

        let _ = mkdirp(&dst).await?;
        let (crypt_major, crypt_minor) = device.get_major_minor();
        let crypt_dev_t = makedev(crypt_major, crypt_minor);
        let _ = mknod(dst, 0660, crypt_dev_t);

        Ok(device)
    }

    async fn mount_partition<'a>(&self, src: impl AsRef<Path>, dst: impl AsRef<Path>, fs: &Filesystem, opt: MountOption<'a>) -> Result<()> {
        if let MountOption::Format { label } = opt {
            let _ = format(fs, &src, label).await?;
        }

        let _ = mkdirp(&dst).await?;
        let _ = mount(fs, src, dst, None::<&str>)?;

        Ok(())
    }

    pub async fn setup(&self) {
        todo!()
    }
}
