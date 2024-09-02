use std::ffi::FromVecWithNulError;
use std::ffi::NulError;

use devicemapper::DmError;
use thiserror::Error;

use crate::app::ApplicationError;
use crate::config::ConfigError;
use crate::dm::crypt::CryptError;
use crate::dm::device::DeviceHandleError;
use crate::key::ring::KeyRingError;
use crate::launcher::dummy::DummyLauncherError;
use crate::launcher::handler::ApplicationHandlerError;
use crate::launcher::oci::OciLauncherError;
use crate::manager::ManagerError;
use crate::util::disk::DiskError;
use crate::util::fs;
use crate::util::net::NetError;
use crate::util::os::OsError;
use crate::util::serde::JsonError;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Application error")]
    ApplicationError(#[from] ApplicationError),

    #[error("Config error")]
    ConfigError(#[from] ConfigError),

    #[error("Manager error")]
    ManagerError(#[from] ManagerError),

    #[error("Filesystem util error")]
    Fs(#[from] fs::FsError),

    #[error("Serde error")]
    Serde(#[from] JsonError),

    #[error("Disk error")]
    Disk(#[from] DiskError),

    #[error("String conversion error to CString")]
    CstringConvError(#[from] NulError),

    #[error("Vector conversion error to CString")]
    CstringFromVecConvError(#[from] FromVecWithNulError),

    #[error("OS error")]
    Os(#[from] OsError),

    #[error("Network util error")]
    NetError(#[from] NetError),

    #[error("Device handle error")]
    DeviceHandleError(#[from] DeviceHandleError),

    #[error("Dm Crypt error")]
    CryptError(#[from] CryptError),

    #[error("Device mapper open error")]
    DmOpenError(#[source] DmError),

    #[error("`{0}` is not a valid device name acording to device mapper")]
    DmInvalidName(String, #[source] devicemapper::DmError),

    #[error("DmUuid conversion error")]
    DmUuidConversionError(#[source] DmError),

    #[error("Cannot create virtual mapping device named: {0}")]
    DmCreateError(String, #[source] devicemapper::DmError),

    #[error("Cannot remove device")]
    DmRemoveDevice(#[source] devicemapper::DmError),

    #[error("Key ring error")]
    KeyRingError(#[from] KeyRingError),

    #[error("Applicatino handler error")]
    HandlerError(#[from] ApplicationHandlerError),

    #[error("Dummy launcher error")]
    DummyLauncherError(#[from] DummyLauncherError),

    #[error("OCI launcher error")]
    OciLauncherError(#[from] OciLauncherError),
}

pub type Result<T> = std::result::Result<T, Error>;
