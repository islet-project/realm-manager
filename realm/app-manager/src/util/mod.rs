use std::{ffi::{CString, FromVecWithNulError, NulError}, path::Path};

use disk::DiskError;
use thiserror::Error;

mod disk;
mod fs;
mod serde;

#[derive(Debug, Error)]
pub enum UtilsError {
    #[error("Filesystem util error")]
    FsError(#[from] fs::FsError),

    #[error("Serde error")]
    SerdeError(#[from] serde::JsonFramedError),

    #[error("Disk error")]
    DiskError(#[from] DiskError),

    #[error("String conversion error to CString")]
    CstringConvError(#[from] NulError),

    #[error("Vector conversion error to CString")]
    CstringFromVecConvError(#[from] FromVecWithNulError)
}

pub type Result<T> = std::result::Result<T, UtilsError>;

pub fn cstring_from_str(v: impl AsRef<str>) -> Result<CString> {
    Ok(CString::new(v.as_ref().as_bytes())?)
}

pub fn cstring_from_path(v: impl AsRef<Path>) -> Result<CString> {
    Ok(CString::new(v.as_ref().as_os_str().as_encoded_bytes())?)
}

pub fn cstring_from_vec(v: Vec<u8>) -> Result<CString> {
    Ok(CString::from_vec_with_nul(v)?)
}
