use nix::{errno::Errno, libc::{sync, LINUX_REBOOT_CMD_POWER_OFF, LINUX_REBOOT_CMD_RESTART}};
use thiserror::Error;
use log::error;

use super::Result;

#[derive(Debug, Error)]
pub enum OsError {
    #[error("Reboot error")]
    RebootError(#[source] Errno)
}

pub enum RebootAction {
    Reboot,
    Shutdown
}

pub fn reboot(action: RebootAction) -> Result<()> {
    let op = match action {
        RebootAction::Reboot => LINUX_REBOOT_CMD_RESTART,
        RebootAction::Shutdown => LINUX_REBOOT_CMD_POWER_OFF
    };

    unsafe {
        nix::libc::sync();
        nix::libc::reboot(op);
    };

    error!("Reboot has failed");

    Err(OsError::RebootError(Errno::last()).into())
}
