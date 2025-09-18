use std::{path::PathBuf, process::Command};

use async_trait::async_trait;
use command_runner::CommandRunner;
use log::debug;
use uuid::Uuid;
use vm_handler::VmHandler;

use crate::{
    managers::vm_manager::{VmManager, VmManagerError, VmStatus},
    storage::app_disk_manager::ApplicationDiskManager,
};

mod command_runner;
pub mod lkvm;
pub mod qemu;
mod vm_handler;

pub struct VmRunner<T: CommandRunner + Sized + Send + Sync> {
    cmd_runner: T,
    realm_id: Uuid,
    realm_workdir: PathBuf,
    vm: Option<VmHandler>,
}

impl<T: CommandRunner + Sized + Send + Sync> VmRunner<T> {
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
    fn handle_get_status(vm: &mut VmHandler) -> Result<VmStatus, VmManagerError> {
        let exit_code = vm
            .try_get_exit_status()
            .map_err(|err| VmManagerError::GetExitCode(err.to_string()))?;
        Ok(if let Some(exit_code) = exit_code {
            VmStatus::Exited(exit_code)
        } else {
            VmStatus::Launched
        })
    }
}

#[async_trait]
impl<T: CommandRunner + Sized + Send + Sync> VmManager for VmRunner<T> {
    async fn launch_vm(&mut self, application_uuids: &[&Uuid]) -> Result<(), VmManagerError> {
        let command = self.prepare_run_command(application_uuids);
        debug!(
            "Spawning realm's vm: {} with command: {:?}",
            self.realm_id, command
        );

        if self.vm.is_some() {
            return Err(VmManagerError::VmAlreadyLaunched);
        }

        let vm_handler = VmHandler::new(command, self.realm_id)
            .await
            .map_err(|err| VmManagerError::Launch(err.to_string()))?;
        self.vm = Some(vm_handler);

        debug!("Realm's vm: {} spawned succesfully!", self.realm_id);
        Ok(())
    }
    async fn shutdown(&mut self) -> Result<(), VmManagerError> {
        debug!("Shutting down realm's vm: {}", self.realm_id);
        self.get_handler()?
            .shutdown()
            .await
            .map_err(|err| VmManagerError::Shutdown(err.to_string()))?;
        self.vm = None;
        debug!("Realm's vm: {} shutdown succesfully!", self.realm_id);
        Ok(())
    }
    fn get_status(&mut self) -> Result<VmStatus, VmManagerError> {
        let status = match self.vm.as_mut() {
            Some(vm_handler) => Self::handle_get_status(vm_handler),
            None => Ok(VmStatus::NotLaunched),
        };
        debug!("Realm's vm: {} status: {:#?}", self.realm_id, status);
        status
    }
}
