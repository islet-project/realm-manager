use std::{
    path::PathBuf,
    process::{Child, Command, ExitStatus},
};

use command_runner::CommandRunner;
use log::trace;
use uuid::Uuid;

use crate::{
    managers::vm_manager::{VmManager, VmManagerError},
    storage::app_disk_manager::ApplicationDiskManager,
};

mod command_runner;
pub mod lkvm;
pub mod qemu;

pub struct VmRunner<T: CommandRunner + Sized> {
    cmd_runner: T,
    realm_workdir: PathBuf,
    vm: Option<Child>,
}

impl<T: CommandRunner + Sized> VmRunner<T> {
    pub fn new(runner: T, realm_workdir: PathBuf) -> Self {
        Self {
            cmd_runner: runner,
            realm_workdir,
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

    fn setup_disk(&self, command: &mut Command, application_uuids: &[&Uuid]) {
        for app_uuid in application_uuids {
            let mut app_disk_path = self.realm_workdir.join(app_uuid.to_string());
            app_disk_path.push(ApplicationDiskManager::DISK_NAME);
            self.cmd_runner.setup_disk(command, &app_disk_path);
        }
    }
}

impl<T: CommandRunner + Sized> VmManager for VmRunner<T> {
    fn launch_vm(&mut self, application_uuids: &[&Uuid]) -> Result<(), VmManagerError> {
        let mut command = Command::new(self.cmd_runner.get_command().get_program());
        command.args(self.cmd_runner.get_command().get_args());
        self.setup_disk(&mut command, application_uuids);
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
