use log::error;
use nix::errno::Errno;
use nix::libc::{LINUX_REBOOT_CMD_POWER_OFF, LINUX_REBOOT_CMD_RESTART};
use thiserror::Error;

use super::Result;

#[derive(Debug, Error)]
pub enum OsError {
    #[error("Reboot error")]
    RebootError(#[source] Errno),
}

pub enum SystemPowerAction {
    Reboot,
    Shutdown,
}

pub fn reboot(action: SystemPowerAction) -> Result<()> {
    let op = match action {
        SystemPowerAction::Reboot => LINUX_REBOOT_CMD_RESTART,
        SystemPowerAction::Shutdown => LINUX_REBOOT_CMD_POWER_OFF,
    };

    unsafe {
        nix::libc::sync();
        nix::libc::reboot(op);
    };

    error!("Reboot has failed");

    Err(OsError::RebootError(Errno::last()).into())
}
