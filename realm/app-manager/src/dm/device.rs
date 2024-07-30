use std::sync::Arc;

use devicemapper::{DevId, DeviceInfo, DmError, DmFlags, DmOptions, DM};
use nix::libc::dev_t;
use thiserror::Error;

use super::Result;

#[derive(Debug, Error)]
pub enum DeviceHandleError {
    #[error("Table load error")]
    TableLoad(#[source] DmError),

    #[error("Resume error")]
    ResumeError(#[source] devicemapper::DmError),

    #[error("Suspend Error")]
    SuspendError(#[source] devicemapper::DmError),

    #[error("The device has no name nor uuid.")]
    NoId()
}

pub struct DeviceHandle {
    dm: Arc<DM>,
    info: DeviceInfo
}

impl DeviceHandle {
    pub fn new(dm: Arc<DM>, info: DeviceInfo) -> Self {
        Self { dm, info }
    }

    pub fn dev_id(&self) -> Result<DevId> {
        Ok(
            self.info.name().map(DevId::Name)
                .or(self.info.uuid().map(DevId::Uuid))
                .ok_or(DeviceHandleError::NoId())?
        )
    }

    pub fn table_load(&self, targets: &[(u64, u64, String, String)], options: Option<DmOptions>) -> Result<()> {
        let id = self.dev_id()?;

        let _ = self.dm.table_load(&id, targets, options.unwrap_or(DmOptions::default()))
            .map_err(DeviceHandleError::TableLoad)?;

        Ok(())
    }
}

pub trait DeviceHandleWrapper {
    fn handle(&self) -> &DeviceHandle;
}

pub trait DeviceHandleWrapperExt: DeviceHandleWrapper {
    fn resume(&self) -> Result<()> {
        let handle = self.handle();
        let id = handle.dev_id()?;

        let _ = handle.dm.device_suspend(&id, DmOptions::default())
            .map_err(DeviceHandleError::ResumeError)?;

        Ok(())
    }

    fn suspend(&self) -> Result<()> {
        let handle = self.handle();
        let id = handle.dev_id()?;

        let _ = handle.dm.device_suspend(&id, DmOptions::default().set_flags(DmFlags::DM_SUSPEND))
            .map_err(DeviceHandleError::SuspendError)?;

        Ok(())
    }

    fn get_major_minor(&self) -> (u32, u32) {
        let handle = self.handle();
        let device = handle.info.device();
        ( device.major, device.minor )
    }
}

impl<T: DeviceHandleWrapper> DeviceHandleWrapperExt for T {}
