use std::path::Path;

use log::error;
use nix::errno::Errno;
use nix::libc::{LINUX_REBOOT_CMD_POWER_OFF, LINUX_REBOOT_CMD_RESTART};
use thiserror::Error;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::task::block_in_place;

use super::cstring_from_str;
use super::Result;

#[derive(Debug, Error)]
pub enum OsError {
    #[error("Reboot error")]
    RebootError(#[source] Errno),

    #[error("Failed to load kernel module, cannot open file")]
    InsmodFileFail(#[source] std::io::Error),

    #[error("Failed to read entire kernel module from disk")]
    InsmodReadFail(#[source] std::io::Error),

    #[error("Failed to insert module")]
    InsmodFail(#[source] Errno),
}

pub enum SystemPowerAction {
    Reboot,
    Shutdown,
}

pub fn reboot(action: SystemPowerAction) -> OsError {
    let op = match action {
        SystemPowerAction::Reboot => LINUX_REBOOT_CMD_RESTART,
        SystemPowerAction::Shutdown => LINUX_REBOOT_CMD_POWER_OFF,
    };

    block_in_place(|| unsafe {
        nix::libc::sync();
        nix::libc::reboot(op);
    });

    error!("Reboot has failed");

    OsError::RebootError(Errno::last())
}

pub async fn insmod(path: impl AsRef<Path>, params: impl AsRef<str>) -> Result<()> {
    let mut file = File::open(path).await.map_err(OsError::InsmodFileFail)?;

    let mut image = Vec::new();
    file.read_to_end(&mut image)
        .await
        .map_err(OsError::InsmodReadFail)?;

    let args = cstring_from_str(params)?;

    block_in_place(|| nix::kmod::init_module(&image, &args).map_err(OsError::InsmodFail))?;

    Ok(())
}
