use std::{os::linux::fs::MetadataExt, path::{self, Path, PathBuf}, process::ExitStatus, sync::Arc};

use nix::libc::{major, makedev, minor, S_IFBLK};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::fs;
use uuid::Uuid;
use log::info;
use warden_realm::ApplicationInfo;

use crate::{app, dm::{crypt::{CryptDevice, CryptoParams, DmCryptTable, Key}, device::{DeviceHandleWrapper, DeviceHandleWrapperExt}, DeviceMapper}, key::{ring::KernelKeyring, KeySealing}, launcher::{ApplicationHandler, Launcher}, util::{disk::read_device_size, fs::{dirname, formatfs, mkdirp, mknod, mount, mount_overlayfs, read_to_string, readlink, stat, umount, write_to_string, Filesystem}, serde::{json_dump, json_load}}};

use super::Result;

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("Partition not found")]
    PartitionNotFound(),

    #[error("Application not provishioned")]
    NotInstalled()
}

#[derive(Serialize, Deserialize)]
pub struct ApplicationMetadata {
    salt: Vec<u8>
}

pub struct Application {
    info: ApplicationInfo,
    workdir: PathBuf,
    devicemapper: Arc<DeviceMapper>,
    keyring: KernelKeyring,
    handler: Option<Box<dyn ApplicationHandler + Send + Sync>>,
    devices: Vec<CryptDevice>
}

impl Application {
    pub fn new(info: ApplicationInfo, workdir: impl ToOwned<Owned = PathBuf>) -> Result<Self> {
        Ok(Self {
            info,
            workdir: workdir.to_owned(),
            devicemapper: Arc::new(DeviceMapper::init()?),
            keyring: KernelKeyring::new(keyutils::SpecialKeyring::User)?,
            handler: None,
            devices: Vec::new()
        })
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
        device.load(
            table,
            &crate::dm::crypt::DevicePath::MajorMinor(major, minor),
            &key,
            None
        )?;
        device.resume()?;

        mkdirp(dirname(&dst).await?).await?;
        let (crypt_major, crypt_minor) = device.get_major_minor();
        let crypt_dev_t = makedev(crypt_major, crypt_minor);
        mknod(dst, 0666|S_IFBLK, crypt_dev_t)?;

        Ok(device)
    }

    async fn try_mount_or_format_partition<'a>(&self, src: impl AsRef<Path>, dst: impl AsRef<Path>, fs: &Filesystem, label: Option<impl AsRef<str>>) -> Result<()> {
        mkdirp(&dst).await?;
        let result = mount(fs, &src, &dst, None::<&str>);

        if let Err(_) = result {
            formatfs(fs, &src, label).await?;
            mount(fs, &src, &dst, None::<&str>)?;
        }

        Ok(())
    }

    fn derive_key_for(&mut self, label: impl AsRef<str>, keyseal: &Box<dyn KeySealing + Send + Sync>, infos: &[&[u8]]) -> Result<Key> {
        const subclass: &'static str = "app-manager";
        let raw_key = keyseal.derive_key(&mut infos.iter())?;
        self.keyring.logon_seal(subclass, &label, &raw_key)?;
        Ok(Key::Keyring { key_size: raw_key.len(), key_type: crate::dm::crypt::KeyType::Logon, key_desc: format!("{}:{}", subclass, label.as_ref()) })
    }

    async fn application_metadata(&self, path: impl AsRef<Path>) -> Result<ApplicationMetadata> {
        let metadata_path = path.as_ref().join("metadata.json");
        let result = read_to_string(&metadata_path).await;

        if let Ok(content) = result {
            Ok(json_load(content)?)
        } else {
            let metadata = ApplicationMetadata {
                salt: Vec::new()
            };

            write_to_string(metadata_path, json_dump(&metadata)?).await?;

            Ok(metadata)
        }
    }

    pub async fn setup(&mut self, params: CryptoParams, mut launcher: Box<dyn Launcher + Send + Sync>, keyseal: Box<dyn KeySealing + Send + Sync>) -> Result<()> {
        let decrypted_partinions_dir = self.workdir.join("crypt");
        let app_image_key = self.derive_key_for(self.info.image_part_uuid.to_string(), &keyseal, &[
            "App manager label".as_bytes()
        ])?;
        let app_image_crypt_device = decrypted_partinions_dir.join("image");
        let device = self.decrypt_partition(&self.info.image_part_uuid, &params, app_image_key, &app_image_crypt_device).await?;
        self.devices.push(device);

        // TODO: Parametrize this
        const fs: Filesystem = Filesystem::Ext2;

        let app_image_dir = self.workdir.join("image");
        self.try_mount_or_format_partition(&app_image_crypt_device, &app_image_dir, &fs, Some("image")).await?;

        let app_image_root_dir = app_image_dir.join("root");
        mkdirp(&app_image_root_dir).await?;
        info!("Installing application");
        launcher.install(&app_image_root_dir, &self.info.name, &self.info.version).await?;

        let mut vendor_data = launcher.read_vendor_data(&app_image_root_dir).await?;
        let app_metadata = self.application_metadata(&app_image_dir).await?;
        vendor_data.push(app_metadata.salt);
        let infos: Vec<_> = vendor_data.iter().map(|i| i.as_slice()).collect();
        let keyseal = keyseal.seal(&mut infos.iter())?;

        let app_name = self.info.name.as_bytes().to_owned();
        let app_data_key = self.derive_key_for(self.info.data_part_uuid.to_string(), &keyseal, &[
            app_name.as_slice()
        ])?;
        let app_data_crypt_device = decrypted_partinions_dir.join("data");
        let device = self.decrypt_partition(&self.info.data_part_uuid, &params, app_data_key, &app_data_crypt_device).await?;
        self.devices.push(device);

        info!("Mounting data partition");
        let app_data_dir = self.workdir.join("data");
        self.try_mount_or_format_partition(&app_data_crypt_device, &app_data_dir, &fs, Some("data")).await?;

        info!("Mounting overlayfs");
        let app_overlay_dir = self.workdir.join("overlay");
        let overlay_lower = app_image_root_dir;
        let overlay_workdir = app_data_dir.join("workdir");
        let overlay_upper = app_data_dir.join("root");

        mkdirp(&app_overlay_dir).await?;
        mkdirp(&overlay_workdir).await?;
        mkdirp(&overlay_upper).await?;

        mount_overlayfs(&overlay_lower, &overlay_upper, &overlay_workdir, &app_overlay_dir)?;

        self.handler = Some(launcher.prepare(&app_overlay_dir).await?);

        Ok(())
    }

    pub fn id(&self) -> &Uuid {
        &self.info.id
    }

    fn get_handler(&mut self) -> Result<&mut (dyn ApplicationHandler + Send + Sync)> {
        Ok(self.handler.as_mut().ok_or(ApplicationError::NotInstalled())?.as_mut())
    }

    pub async fn start(&mut self) -> Result<()> {
        self.get_handler()?.start().await?;

        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        self.get_handler()?.stop().await?;

        Ok(())

    }

    pub async fn kill(&mut self) -> Result<()> {
        self.get_handler()?.kill().await?;

        Ok(())
    }

    pub async fn wait(&mut self) -> Result<ExitStatus> {
        let status = self.get_handler()?.wait().await?;

        Ok(status)
    }

    pub async fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        let status = self.get_handler()?.try_wait().await?;

        Ok(status)
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(handler) = self.handler.as_mut() {
            info!("Stopping {:?}", self.info.id);
            handler.stop().await?;
        }
        self.handler = None;

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
