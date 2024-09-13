use std::{
    path::PathBuf,
    process::{Command, ExitStatus},
};

use command_runner::CommandRunner;
use log::trace;
use uuid::Uuid;
use vm_handler::VmHandler;

use crate::{
    managers::vm_manager::{VmManager, VmManagerError},
    storage::app_disk_manager::ApplicationDiskManager,
};

mod command_runner;
pub mod lkvm;
pub mod qemu;
mod vm_handler;

pub struct VmRunner<T: CommandRunner + Sized> {
    cmd_runner: T,
    realm_id: Uuid,
    realm_workdir: PathBuf,
    vm: Option<VmHandler>,
}

impl<T: CommandRunner + Sized> VmRunner<T> {
    pub fn new(runner: T, realm_id: Uuid, realm_workdir: PathBuf) -> Self {
        Self {
            cmd_runner: runner,
            realm_id,
            realm_workdir,
            vm: None,
        }
    }

    fn get_handler(&mut self) -> Result<&mut VmHandler, VmManagerError> {
        self.vm.as_mut().ok_or(VmManagerError::VmNotLaunched)
    }

    fn prepare_run_command(&self, application_uuids: &[&Uuid]) -> Command {
        let mut command = Command::new(self.cmd_runner.get_command().get_program());
        command.args(self.cmd_runner.get_command().get_args());

        for app_uuid in application_uuids {
            let mut app_disk_path = self.realm_workdir.join(app_uuid.to_string());
            app_disk_path.push(ApplicationDiskManager::DISK_NAME);
            self.cmd_runner.setup_disk(&mut command, &app_disk_path);
        }
        command
    }
}

impl<T: CommandRunner + Sized> VmManager for VmRunner<T> {
    fn launch_vm(&mut self, application_uuids: &[&Uuid]) -> Result<(), VmManagerError> {
        let command = self.prepare_run_command(application_uuids);
        trace!("Spawning realm with command: {:?}", command);

        match self.vm.as_mut() {
            Some(_) => Err(VmManagerError::VmAlreadyLaunched),
            None => {
                self.vm = Some(
                    VmHandler::new(command, self.realm_id)
                        .map_err(|err| VmManagerError::Launch(err.to_string()))?,
                );
                Ok(())
            }
        }
    }
    fn shutdown(&mut self) -> Result<(), VmManagerError> {
        self.get_handler()?
            .shutdown()
            .map_err(|err| VmManagerError::Shutdown(err.to_string()))
    }
    fn try_get_exit_status(&mut self) -> Result<Option<ExitStatus>, VmManagerError> {
        self.get_handler()?
            .try_get_exit_status()
            .map_err(|err| VmManagerError::GetExitCode(err.to_string()))
    }
}
