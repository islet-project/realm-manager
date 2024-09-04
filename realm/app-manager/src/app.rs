use std::os::linux::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use log::info;
use nix::libc::{major, makedev, minor, S_IFBLK};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use warden_realm::ApplicationInfo;

use super::Result;
use crate::dm::crypt::{CryptDevice, CryptoParams, DmCryptTable, Key};
use crate::dm::device::{DeviceHandleWrapper, DeviceHandleWrapperExt};
use crate::dm::DeviceMapper;
use crate::key::ring::KernelKeyring;
use crate::key::KeySealing;
use crate::launcher::{ApplicationHandler, Launcher};
use crate::util::disk::read_device_size;
use crate::util::fs::{
    dirname, formatfs, mkdirp, mknod, mount, mount_overlayfs, stat, umount,
    Filesystem,
};

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("Partition not found")]
    PartitionNotFound(Uuid),
}

#[derive(Serialize, Deserialize)]
pub struct ApplicationMetadata {
    salt: Vec<u8>,
}

pub struct Application {
    info: ApplicationInfo,
    workdir: PathBuf,
    devicemapper: Arc<DeviceMapper>,
    keyring: KernelKeyring,
    devices: Vec<CryptDevice>,
}

impl Application {
    pub fn new(info: ApplicationInfo, workdir: PathBuf) -> Result<Self> {
        Ok(Self {
            info,
            workdir,
            devicemapper: Arc::new(DeviceMapper::init()?),
            keyring: KernelKeyring::new(keyutils::SpecialKeyring::User)?,
            devices: Vec::new(),
        })
    }

    async fn decrypt_partition(
        &self,
        part_uuid: &Uuid,
        params: &CryptoParams,
        key: Key,
        dst: impl AsRef<Path>,
    ) -> Result<CryptDevice> {
        let uuid_str = part_uuid.to_string();
        let path = Path::new("/dev/disk/by-partuuid/").join(&uuid_str);

        if !path.is_symlink() {
            return Err(ApplicationError::PartitionNotFound(*part_uuid).into());
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
            offset: 0,
        };

        let device: CryptDevice =
            self.devicemapper
                .create_device(device_name, None::<Uuid>, None)?;
        device.load(
            table,
            &crate::dm::crypt::DevicePath::MajorMinor(major, minor),
            &key,
            None,
        )?;
        device.resume()?;

        mkdirp(dirname(&dst).await?).await?;
        let (crypt_major, crypt_minor) = device.get_major_minor();
        let crypt_dev_t = makedev(crypt_major, crypt_minor);
        mknod(dst, 0o666 | S_IFBLK, crypt_dev_t)?;

        Ok(device)
    }

    async fn try_mount_or_format_partition<'a>(
        &self,
        src: impl AsRef<Path>,
        dst: impl AsRef<Path>,
        fs: &Filesystem,
        label: Option<impl AsRef<str>>,
    ) -> Result<()> {
        mkdirp(&dst).await?;
        let result = mount(fs, &src, &dst, None::<&str>);

        if result.is_err() {
            formatfs(fs, &src, label).await?;
            mount(fs, &src, &dst, None::<&str>)?;
        }

        Ok(())
    }

    fn derive_key_for(
        &mut self,
        label: impl AsRef<str>,
        keyseal: &(dyn KeySealing + Send + Sync),
        infos: &[&[u8]],
    ) -> Result<Key> {
        const SUBCLASS: &str = "app-manager";
        let raw_key = keyseal.derive_key(infos)?;
        self.keyring.logon_seal(SUBCLASS, &label, &raw_key)?;
        Ok(Key::Keyring {
            key_size: raw_key.len(),
            key_type: crate::dm::crypt::KeyType::Logon,
            key_desc: format!("{}:{}", SUBCLASS, label.as_ref()),
        })
    }

    pub async fn setup(
        &mut self,
        params: CryptoParams,
        mut launcher: Box<dyn Launcher + Send + Sync>,
        keyseal: Box<dyn KeySealing + Send + Sync>,
    ) -> Result<Box<dyn ApplicationHandler + Send + Sync>> {
        let decrypted_partinions_dir = self.workdir.join("crypt");
        let app_image_key = self.derive_key_for(
            self.info.image_part_uuid.to_string(),
            keyseal.as_ref(),
            &["App manager label".as_bytes()],
        )?;
        let app_image_crypt_device = decrypted_partinions_dir.join("image");
        let device = self
            .decrypt_partition(
                &self.info.image_part_uuid,
                &params,
                app_image_key,
                &app_image_crypt_device,
            )
            .await?;
        self.devices.push(device);

        // TODO: Parametrize this
        const FS: Filesystem = Filesystem::Ext2;

        let app_image_dir = self.workdir.join("image");
        self.try_mount_or_format_partition(
            &app_image_crypt_device,
            &app_image_dir,
            &FS,
            Some("image"),
        )
        .await?;

        let app_image_root_dir = app_image_dir.join("root");
        mkdirp(&app_image_root_dir).await?;
        info!("Installing application");
        let application_metadata = launcher
            .install(
                &app_image_root_dir,
                &self.info.image_registry,
                &self.info.name,
                &self.info.version,
            )
            .await?;

        let infos: Vec<_> = application_metadata.vendor_data.iter().map(|i| i.as_slice()).collect();
        let keyseal = keyseal.seal(&infos, &application_metadata.image_hash)?;

        let app_name = self.info.name.as_bytes().to_owned();
        let app_data_key = self.derive_key_for(
            self.info.data_part_uuid.to_string(),
            keyseal.as_ref(),
            &[app_name.as_slice()],
        )?;
        let app_data_crypt_device = decrypted_partinions_dir.join("data");
        let device = self
            .decrypt_partition(
                &self.info.data_part_uuid,
                &params,
                app_data_key,
                &app_data_crypt_device,
            )
            .await?;
        self.devices.push(device);

        info!("Mounting data partition");
        let app_data_dir = self.workdir.join("data");
        self.try_mount_or_format_partition(
            &app_data_crypt_device,
            &app_data_dir,
            &FS,
            Some("data"),
        )
        .await?;

        info!("Mounting overlayfs");
        let app_overlay_dir = self.workdir.join("overlay");
        let overlay_lower = app_image_root_dir;
        let overlay_workdir = app_data_dir.join("workdir");
        let overlay_upper = app_data_dir.join("root");

        mkdirp(&app_overlay_dir).await?;
        mkdirp(&overlay_workdir).await?;
        mkdirp(&overlay_upper).await?;

        mount_overlayfs(
            &overlay_lower,
            &overlay_upper,
            &overlay_workdir,
            &app_overlay_dir,
        )?;

        launcher.prepare(&app_overlay_dir).await
    }

    pub fn id(&self) -> &Uuid {
        &self.info.id
    }

    pub async fn cleanup(&mut self) -> Result<()> {
        let app_image_dir = self.workdir.join("image");
        let app_data_dir = self.workdir.join("data");
        let app_overlay_dir = self.workdir.join("overlay");

        for dir in [app_overlay_dir, app_data_dir, app_image_dir].into_iter() {
            info!("Unmounting: {:?}", dir);
            umount(dir)?;
        }

        while let Some(device) = self.devices.pop() {
            info!("Remove dm crypt device: {:?}", device.handle().dev_id()?);
            device.suspend()?;
            self.devicemapper.remove_device(device, None)?;
        }

        Ok(())
    }
}
