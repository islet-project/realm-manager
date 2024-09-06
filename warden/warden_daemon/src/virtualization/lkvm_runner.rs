use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus};

use log::debug;
use thiserror::Error;
use uuid::Uuid;

use crate::managers::realm_configuration::{
    CpuConfig, KernelConfig, MemoryConfig, NetworkConfig, RealmConfig,
};
use crate::managers::vm_manager::{VmManager, VmManagerError};
use crate::storage::app_disk_manager::ApplicationDiskManager;

#[derive(Debug, Error)]
pub enum LkvmError {
    #[error("Missing param: {0}")]
    MissingObligatoryParam(String),
}

pub struct LkvmRunner {
    realm_workdir: PathBuf,
    command: Command,
    vm: Option<Child>,
}

impl LkvmRunner {
    pub fn new(
        path_to_runner: PathBuf,
        realm_workdir: PathBuf,
        config: &RealmConfig,
    ) -> Result<Self, VmManagerError> {
        let mut runner = LkvmRunner {
            realm_workdir,
            command: Command::new(path_to_runner),
            vm: None,
        };
        runner.setup_cpu(&config.cpu);
        runner
            .setup_kernel(&config.kernel)
            .map_err(|err| VmManagerError::Create(err.to_string()))?;
        runner.setup_memory(&config.memory);
        runner.setup_network(&config.network);
        runner.control_output();
        Ok(runner)
    }
    pub fn configure_realm_settings(&mut self) {
        self.command.arg("--irqchip=gicv3");
        self.command.arg("--disable-sve");
        self.command.arg("--debug");
        self.command.arg("--realm");
        self.command.arg("--measurement-algo=\"sha256\"");
    }
    fn setup_network(&mut self, config: &NetworkConfig) {
        self.command.arg("-n").arg("virtio");
    }
    fn setup_kernel(&mut self, config: &KernelConfig) -> Result<(), LkvmError> {
        self.command.arg("-k").arg(&config.kernel_path);
        if let Some(kernel_cmd_params) = &config.kernel_cmd_params {
            self.command
                .arg("-p")
                .arg(&format!("\"{}\"", kernel_cmd_params));
        }
        Ok(())
    }
    fn setup_cpu(&mut self, config: &CpuConfig) {
        self.command.arg("-c").arg(config.cores_number.to_string());
    }
    fn setup_memory(&mut self, config: &MemoryConfig) {
        self.command.arg("-m").arg(config.ram_size.to_string());
    }
    fn control_output(&mut self) {
        self.command.arg("--console").arg("serial");
    }
    fn setup_disk(&self, command: &mut Command, application_uuids: &[&Uuid]) {
        for app_uuid in application_uuids {
            let mut app_disk_path = self.realm_workdir.join(app_uuid.to_string());
            app_disk_path.push(ApplicationDiskManager::DISK_NAME);
            command
                .arg("-drive")
                .arg(format!("file={}", app_disk_path.to_string_lossy()));
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

impl VmManager for LkvmRunner {
    fn launch_vm(&mut self, application_uuids: &[&Uuid]) -> Result<(), VmManagerError> {
        let mut command = Command::new(self.command.get_program());
        // command.args(self.command.get_args());
        // self.setup_disk(&mut command, application_uuids);
        debug!("Spawning realm with command: {:?}", command);
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
