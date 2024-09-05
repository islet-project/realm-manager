use std::ffi::FromVecWithNulError;
use std::ffi::NulError;

use devicemapper::DmError;
use thiserror::Error;

use crate::app::ApplicationError;
use crate::config::ConfigError;
use crate::dm::crypt::CryptError;
use crate::dm::device::DeviceHandleError;
use crate::key::hkdf::HkdfSealingError;
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
use crate::util::token::RsiTokenResolverError;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Application error")]
    Application(#[from] ApplicationError),

    #[error("Config error")]
    Config(#[from] ConfigError),

    #[error("Manager error")]
    Manager(#[from] ManagerError),

    #[error("Filesystem util error")]
    Fs(#[from] fs::FsError),

    #[error("Serde error")]
    Serde(#[from] JsonError),

    #[error("Disk error")]
    Disk(#[from] DiskError),

    #[error("String conversion error to CString")]
    CstringConv(#[from] NulError),

    #[error("Vector conversion error to CString")]
    CstringFromVecConv(#[from] FromVecWithNulError),

    #[error("OS error")]
    Os(#[from] OsError),

    #[error("Network util error")]
    Net(#[from] NetError),

    #[error("Device handle error")]
    DeviceHandle(#[from] DeviceHandleError),

    #[error("Dm Crypt error")]
    Crypt(#[from] CryptError),

    #[error("Device mapper open error")]
    DmOpen(#[source] DmError),

    #[error("`{0}` is not a valid device name acording to device mapper")]
    DmInvalidName(String, #[source] devicemapper::DmError),

    #[error("DmUuid conversion error")]
    DmUuidConversion(#[source] DmError),

    #[error("Cannot create virtual mapping device named: {0}")]
    DmCreate(String, #[source] devicemapper::DmError),

    #[error("Cannot remove device")]
    DmRemoveDevice(#[source] devicemapper::DmError),

    #[error("Key ring error")]
    KeyRing(#[from] KeyRingError),

    #[error("Applicatino handler error")]
    Handler(#[from] ApplicationHandlerError),

    #[error("Dummy launcher error")]
    DummyLauncher(#[from] DummyLauncherError),

    #[error("OCI launcher error")]
    OciLauncher(#[from] OciLauncherError),

    #[error("Hkdf sealing error")]
    HkdfSealing(#[from] HkdfSealingError),

    #[error("Rsi token resolver error")]
    RsiTokenResolver(#[from] RsiTokenResolverError),
}

pub type Result<T> = std::result::Result<T, Error>;
