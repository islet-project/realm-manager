use std::{ffi::c_void, fmt, os::unix::ffi::OsStrExt, path::Path};
use nix::{errno::Errno, libc::{c_char, dev_t, mode_t}};
use thiserror::Error;
use tokio::process::Command;
use super::{cstring_from_path, cstring_from_str, cstring_from_vec, Result};

#[derive(Debug, Error)]
pub enum FsError {
    #[error("Failed to start mkfs command")]
    MkfsStartFailure(#[source] std::io::Error),

    #[error("Failed to format drive")]
    MkfsFormatError(#[source] std::io::Error),

    #[error("Filesystem mounting error")]
    MountError(#[source] Errno),

    #[error("Error creating device file")]
    MknodError(#[source] Errno)
}

pub enum Filesystem {
    Ext2,
    Ext3,
    Ext4
}

impl fmt::Display for Filesystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Filesystem::Ext2 => write!(f, "ext2"),
            Filesystem::Ext3 => write!(f, "ext3"),
            Filesystem::Ext4 => write!(f, "ext4"),
        }
    }
}

fn check_libc_error(result: i32, f: impl FnOnce(Errno) -> FsError) -> Result<()> {
    if result != 0 {
        Err(f(Errno::last()).into())
    } else {
        Ok(())
    }
}

pub async fn format(ty: Filesystem, device: impl AsRef<Path>, label: Option<impl AsRef<str>>) -> Result<()> {
    let mut cmd = Command::new(format!("/sbin/mkfs.{}", ty));

    if let Some(l) = label.as_ref() {
        cmd.arg("-L").arg(l.as_ref());
    }

    cmd.arg(device.as_ref());

    let mut child = cmd.spawn()
        .map_err(FsError::MkfsStartFailure)?;

    child.wait()
        .await
        .map_err(FsError::MkfsFormatError)?;

    Ok(())
}

pub fn mount(fs: Filesystem, src: impl AsRef<Path>, dst: impl AsRef<Path>, opt: Option<impl AsRef<str>>) -> Result<()> {
    let fs = cstring_from_str(fs.to_string())?;
    let src = cstring_from_path(src)?;
    let dst = cstring_from_path(dst)?;
    let opt = opt.map(|i| cstring_from_str(i)).transpose()?;

    let result = unsafe {
        nix::libc::mount(
            src.as_ptr() as *const c_char,
            dst.as_ptr() as *const c_char,
            fs.as_ptr() as *const c_char,
            0,
            opt.map_or(std::ptr::null(), |i| i.as_ptr() as *const c_void)
        )
    };

    check_libc_error(result, FsError::MountError)
}

pub fn mount_overlayfs(lower: impl AsRef<Path>, upper: impl AsRef<Path>, workdir: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<()> {
    let fs = cstring_from_str("overlay")?;
    let opt = cstring_from_vec([
        b"lowerdir=", lower.as_ref().as_os_str().as_bytes(), b",",
        b"upperdir=", upper.as_ref().as_os_str().as_bytes(), b",",
        b"workdir=", workdir.as_ref().as_os_str().as_bytes()
    ].concat())?;
    let dst = cstring_from_path(dst)?;

    let result = unsafe {
        nix::libc::mount(
            fs.as_ptr() as *const c_char,
            dst.as_ptr() as *const c_char,
            fs.as_ptr() as *const c_char,
            0,
            opt.as_ptr() as *const c_void
        )
    };

    check_libc_error(result, FsError::MountError)
}

pub fn mknod(path: impl AsRef<Path>, mode: mode_t, dev: dev_t) -> Result<()> {
    let path = cstring_from_path(path)?;

    let result = unsafe {
        nix::libc::mknod(
            path.as_ptr() as *const c_char,
            mode,
            dev
        )
    };

    check_libc_error(result, FsError::MknodError)
}

