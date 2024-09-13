use std::{
    io,
    process::{Child, Command, ExitStatus},
};

use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum VmHandlerError {
    #[error("Unable to spawn Vm: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("Unable to launch Vm: {0}")]
    Launch(ExitStatus),
    #[error("Unable to kill Vm: {0}")]
    Kill(#[source] std::io::Error),
    #[error("Unable to get realm's exit code: {0}")]
    Wait(#[source] std::io::Error),
}

pub struct VmHandler {
    vm_process: Child,
    vm_id: Uuid,
}

impl VmHandler {
    pub fn new(mut command: Command, vm_id: Uuid) -> Result<VmHandler, VmHandlerError> {
        let mut vm_process = command.spawn().map_err(VmHandlerError::Spawn)?;
        match vm_process.try_wait().map_err(VmHandlerError::Wait)? {
            Some(exit_status) => Err(VmHandlerError::Launch(exit_status)),
            None => Ok(VmHandler { vm_process, vm_id }),
        }
    }

    pub fn shutdown(&mut self) -> Result<(), VmHandlerError> {
        self.vm_process.kill().map_err(VmHandlerError::Kill)?;
        self.vm_process
            .wait()
            .map(|_| ())
            .map_err(VmHandlerError::Wait)
    }

    pub fn try_get_exit_status(&mut self) -> Result<Option<ExitStatus>, io::Error> {
        self.vm_process.try_wait()
    }
}
