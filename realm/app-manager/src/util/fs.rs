use std::ffi::c_void;
use std::fmt;
use std::fs::Metadata;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use nix::errno::Errno;
use nix::libc::{c_char, dev_t, mode_t};
use thiserror::Error;
use tokio::task::block_in_place;
use tokio::{fs, process::Command};

use super::{cstring_from_path, cstring_from_str, cstring_from_vec, Result};

#[derive(Debug, Error)]
pub enum FsError {
    #[error("Failed to start mkfs command")]
    MkfsStartFailure(#[source] std::io::Error),

    #[error("Failed to format drive")]
    MkfsFormatError(#[source] std::io::Error),

    #[error("Filesystem mounting error")]
    MountError(#[source] Errno),

    #[error("Failed to umount filesystem")]
    UmountError(#[source] Errno),

    #[error("Error creating device file")]
    MknodError(#[source] Errno),

    #[error("Failed to create all directories")]
    MkdirpError(#[source] std::io::Error),

    #[error("Error while reading link")]
    ReadLinkError(#[source] std::io::Error),

    #[error("Stat error")]
    StatError(#[source] std::io::Error),

    #[error("File read error")]
    FileReadError(#[source] std::io::Error),

    #[error("File write error")]
    FileWriteError(#[source] std::io::Error),

    #[error("Path has not parent")]
    PathHasNoParent(),

    #[error("Failed to remove directory with content")]
    RmRfError(#[source] std::io::Error),

    #[error("Failed to move file or directory")]
    RenameError(#[source] std::io::Error),
}

#[allow(dead_code)]
pub enum Filesystem {
    Ext2,
    Ext3,
    Ext4,
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

pub async fn formatfs(
    ty: &Filesystem,
    device: impl AsRef<Path>,
    label: Option<impl AsRef<str>>,
) -> Result<()> {
    let mut cmd = Command::new(format!("/sbin/mkfs.{}", ty));

    if let Some(l) = label.as_ref() {
        cmd.arg("-L").arg(l.as_ref());
    }

    cmd.arg(device.as_ref());

    let mut child = cmd.spawn().map_err(FsError::MkfsStartFailure)?;

    child.wait().await.map_err(FsError::MkfsFormatError)?;

    Ok(())
}

pub fn mount(
    fs: &Filesystem,
    src: impl AsRef<Path>,
    dst: impl AsRef<Path>,
    opt: Option<impl AsRef<str>>,
) -> Result<()> {
    let fs = cstring_from_str(fs.to_string())?;
    let src = cstring_from_path(src)?;
    let dst = cstring_from_path(dst)?;
    let opt = opt.map(|i| cstring_from_str(i)).transpose()?;

    let result = block_in_place(|| unsafe {
        nix::libc::mount(
            src.as_ptr() as *const c_char,
            dst.as_ptr() as *const c_char,
            fs.as_ptr() as *const c_char,
            0,
            opt.map_or(std::ptr::null(), |i| i.as_ptr() as *const c_void),
        )
    });

    check_libc_error(result, FsError::MountError)
}

pub fn umount(path: impl AsRef<Path>) -> Result<()> {
    let path = cstring_from_path(path)?;

    let result = block_in_place(|| unsafe { nix::libc::umount(path.as_ptr() as *const c_char) });

    check_libc_error(result, FsError::UmountError)
}

pub fn mount_overlayfs(
    lower: impl AsRef<Path>,
    upper: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    dst: impl AsRef<Path>,
) -> Result<()> {
    let fs = cstring_from_str("overlay")?;
    let opt = cstring_from_vec(
        [
            b"lowerdir=",
            lower.as_ref().as_os_str().as_bytes(),
            b",",
            b"upperdir=",
            upper.as_ref().as_os_str().as_bytes(),
            b",",
            b"workdir=",
            workdir.as_ref().as_os_str().as_bytes(),
            b"\x00",
        ]
        .concat(),
    )?;
    let dst = cstring_from_path(dst)?;

    let result = block_in_place(|| unsafe {
        nix::libc::mount(
            fs.as_ptr() as *const c_char,
            dst.as_ptr() as *const c_char,
            fs.as_ptr() as *const c_char,
            0,
            opt.as_ptr() as *const c_void,
        )
    });

    check_libc_error(result, FsError::MountError)
}

pub fn mknod(path: impl AsRef<Path>, mode: mode_t, dev: dev_t) -> Result<()> {
    let path = cstring_from_path(path)?;

    let result =
        block_in_place(|| unsafe { nix::libc::mknod(path.as_ptr() as *const c_char, mode, dev) });

    check_libc_error(result, FsError::MknodError)
}

pub async fn mkdirp(path: impl AsRef<Path>) -> Result<()> {
    fs::create_dir_all(path)
        .await
        .map_err(FsError::MkdirpError)?;

    Ok(())
}

pub async fn readlink(path: impl AsRef<Path>) -> Result<PathBuf> {
    Ok(fs::read_link(path).await.map_err(FsError::ReadLinkError)?)
}

pub async fn stat(path: impl AsRef<Path>) -> Result<Metadata> {
    Ok(fs::metadata(path).await.map_err(FsError::StatError)?)
}

pub async fn read_to_string(path: impl AsRef<Path>) -> Result<String> {
    Ok(fs::read_to_string(path)
        .await
        .map_err(FsError::FileReadError)?)
}

pub async fn read_to_vec(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    Ok(fs::read(path)
        .await
        .map_err(FsError::FileReadError)?)
}

pub async fn write_to_file(path: impl AsRef<Path>, content: impl AsRef<[u8]>) -> Result<()> {
    fs::write(path, content)
        .await
        .map_err(FsError::FileWriteError)?;

    Ok(())
}

pub async fn dirname(path: impl AsRef<Path>) -> Result<PathBuf> {
    Ok(path
        .as_ref()
        .parent()
        .ok_or(FsError::PathHasNoParent())?
        .to_owned())
}

pub async fn rmrf(path: impl AsRef<Path>) -> Result<()> {
    fs::remove_dir_all(path).await.map_err(FsError::RmRfError)?;

    Ok(())
}

pub async fn rename(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<()> {
    fs::rename(src, dst).await.map_err(FsError::RenameError)?;

    Ok(())
}
