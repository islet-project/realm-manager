use log::error;
use nix::errno::Errno;
use nix::libc::{LINUX_REBOOT_CMD_POWER_OFF, LINUX_REBOOT_CMD_RESTART};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OsError {
    #[error("Reboot error")]
    RebootError(#[source] Errno),
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

    unsafe {
        nix::libc::sync();
        nix::libc::reboot(op);
    };

    error!("Reboot has failed");

    OsError::RebootError(Errno::last())
}
