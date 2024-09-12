use std::sync::Arc;

use devicemapper::{DmName, DmOptions, DmUuid, DM};
use tokio::task::block_in_place;
use uuid::Uuid;

use device::{DeviceHandle, DeviceHandleWrapper};

pub mod crypt;
pub mod device;

use crate::error::{Error, Result};

pub struct DeviceMapper(Arc<DM>);

impl DeviceMapper {
    pub fn init() -> Result<Self> {
        Ok(Self(Arc::new(DM::new().map_err(Error::DmOpen)?)))
    }

    fn create_device_handle(
        &self,
        name: impl AsRef<str>,
        uuid: Option<impl AsRef<Uuid>>,
        opt: Option<DmOptions>,
    ) -> Result<DeviceHandle> {
        let name = DmName::new(name.as_ref())
            .map_err(|e| Error::DmInvalidName(name.as_ref().to_owned(), e))?;
        let opt = opt.unwrap_or_default();

        let result = if let Some(uuid) = uuid {
            let uuid_str = uuid.as_ref().to_string();
            let dm_uuid = DmUuid::new(&uuid_str).map_err(Error::DmUuidConversion)?;

            block_in_place(|| self.0.device_create(name, Some(dm_uuid), opt))
        } else {
            block_in_place(|| self.0.device_create(name, None, opt))
        };

        let info = result.map_err(|e| Error::DmCreate(name.to_string(), e))?;

        Ok(DeviceHandle::new(self.0.clone(), info))
    }

    pub fn create_device<T: From<DeviceHandle>>(
        &self,
        name: impl AsRef<str>,
        uuid: Option<impl AsRef<Uuid>>,
        opt: Option<DmOptions>,
    ) -> Result<T> {
        Ok(T::from(self.create_device_handle(name, uuid, opt)?))
    }

    pub fn remove_device(
        &self,
        device: impl DeviceHandleWrapper,
        opt: Option<DmOptions>,
    ) -> Result<()> {
        block_in_place(|| {
            self.0
                .device_remove(&device.handle().dev_id()?, opt.unwrap_or_default())
                .map_err(Error::DmRemoveDevice)
        })?;

        Ok(())
    }
}
