use std::process::{Child, Command, ExitStatus};

use command_runner::CommandRunner;
use log::trace;
use uuid::Uuid;

use crate::managers::vm_manager::{VmManager, VmManagerError};

mod command_runner;
pub mod lkvm;
pub mod qemu;

pub struct VmRunner<T: CommandRunner + Sized> {
    cmd_runner: T,
    vm: Option<Child>,
}

impl<T: CommandRunner + Sized> VmRunner<T> {
    pub fn new(runner: T) -> Self {
        Self {
            cmd_runner: runner,
            vm: None,
        }
    }

    fn kill_and_wait(child: &mut Child) -> Result<(), VmManagerError> {
        child
            .kill()
            .map_err(|err| VmManagerError::Destroy(err.to_string()))?;
        child
            .wait()
            .map(|_| ())
            .map_err(|err| VmManagerError::Destroy(err.to_string()))
    }
}

impl<T: CommandRunner + Sized> VmManager for VmRunner<T> {
    fn launch_vm(&mut self, application_uuids: &[&Uuid]) -> Result<(), VmManagerError> {
        let mut command = Command::new(self.cmd_runner.get_command().get_program());
        command.args(self.cmd_runner.get_command().get_args());
        self.cmd_runner.setup_disk(&mut command, application_uuids);
        trace!("Spawning realm with command: {:?}", command);
        command
            .spawn()
            .map(|child| {
                self.vm = Some(child);
            })
            .map_err(VmManagerError::Launch)
    }
    fn stop_vm(&mut self) -> Result<(), VmManagerError> {
        self.vm
            .as_mut()
            .map(|child| child.kill().map_err(|_| VmManagerError::Stop))
            .unwrap_or(Err(VmManagerError::VmNotLaunched))
    }
    fn delete_vm(&mut self) -> Result<(), VmManagerError> {
        self.vm
            .as_mut()
            .map(Self::kill_and_wait)
            .unwrap_or(Err(VmManagerError::VmNotLaunched))
    }
    fn get_exit_status(&mut self) -> Option<ExitStatus> {
        if let Some(vm) = &mut self.vm {
            return vm.try_wait().ok()?;
        }
        None
    }
}
