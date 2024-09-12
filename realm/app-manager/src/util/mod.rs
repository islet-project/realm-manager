use std::ffi::CString;
use std::path::Path;

use crate::error::Result;

pub mod disk;
pub mod fs;
pub mod net;
pub mod os;
pub mod serde;
pub mod token;

pub fn cstring_from_str(v: impl AsRef<str>) -> Result<CString> {
    Ok(CString::new(v.as_ref().as_bytes())?)
}

pub fn cstring_from_path(v: impl AsRef<Path>) -> Result<CString> {
    Ok(CString::new(v.as_ref().as_os_str().as_encoded_bytes())?)
}

pub fn cstring_from_vec(v: Vec<u8>) -> Result<CString> {
    Ok(CString::from_vec_with_nul(v)?)
}
