use std::{borrow::Borrow, sync::Arc};

use crypt::CryptError;
use device::{DeviceHandle, DeviceHandleError};
use devicemapper::{DmError, DmName, DmOptions, DmUuid, DmUuidBuf, DM};
use thiserror::Error;
use uuid::Uuid;

pub mod crypt;
pub mod device;

#[derive(Debug, Error)]
pub enum DeviceMapperError {
    #[error("Device handle error")]
    DeviceHandleError(#[from] DeviceHandleError),

    #[error("Dm Crypt error")]
    CryptError(#[from] CryptError),

    #[error("Device mapper open error")]
    OpenError(#[source] DmError),

    #[error("`{0}` is not a valid device name acording to device mapper")]
    InvalidName(String, #[source] devicemapper::DmError),

    #[error("DmUuid conversion error")]
    DmUuidConversionError(#[source] DmError),

    #[error("Cannot create virtual mapping device named: {0}")]
    CreateError(String, #[source] devicemapper::DmError),
}

pub type Result<T> = std::result::Result<T, DeviceMapperError>;

pub struct DeviceMapper(Arc<DM>);

impl DeviceMapper {
    pub fn init() -> Result<Self> {
        Ok(Self (Arc::new(DM::new().map_err(DeviceMapperError::OpenError)?)))
    }

    fn create_device_handle(&self, name: impl AsRef<str>, uuid: Option<impl AsRef<Uuid>>, opt: Option<DmOptions>) -> Result<DeviceHandle> {
        let name = DmName::new(name.as_ref())
            .map_err(|e| DeviceMapperError::InvalidName(name.as_ref().to_owned(), e))?;
        let opt = opt.unwrap_or(DmOptions::default());

        let result = if let Some(uuid) = uuid {
            let uuid_str = uuid.as_ref().to_string();
            let dm_uuid = DmUuid::new(&uuid_str)
                .map_err(DeviceMapperError::DmUuidConversionError)?;

            self.0.device_create(name, Some(&dm_uuid), opt)
        } else {
            self.0.device_create(name, None, opt)
        };

        let info = result.map_err(|e| DeviceMapperError::CreateError(name.to_string(), e))?;

        Ok(DeviceHandle::new(self.0.clone(), info))
    }

    pub fn create_device<T: From<DeviceHandle>>(&self, name: impl AsRef<str>, uuid: Option<impl AsRef<Uuid>>, opt: Option<DmOptions>) -> Result<T> {
        Ok(T::from(self.create_device_handle(name, uuid, opt)?))
    }
}
