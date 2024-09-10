use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

use crate::managers::realm_configuration::{
    CpuConfig, KernelConfig, MemoryConfig, NetworkConfig, RealmConfig,
};
use crate::storage::app_disk_manager::ApplicationDiskManager;

use super::command_runner::CommandRunner;
pub struct LkvmRunner {
    realm_workdir: PathBuf,
    command: Command,
}

impl LkvmRunner {
    pub fn new(path_to_runner: PathBuf, realm_workdir: PathBuf, config: &RealmConfig) -> Self {
        let mut runner = LkvmRunner {
            realm_workdir,
            command: Command::new(path_to_runner),
        };
        runner.command.arg("run");
        runner.setup_cpu(&config.cpu);
        runner.setup_kernel(&config.kernel);
        runner.setup_memory(&config.memory);
        runner.setup_network(&config.network);
        runner.control_output();
        runner
    }
    pub fn configure_cca_settings(&mut self) {
        self.command.arg("--debug");
        self.command.arg("--irqchip=gicv3");
        self.command.arg("--disable-sve");
        self.command.arg("--realm");
        self.command.arg("--measurement-algo=sha256");
    }
    fn setup_network(&mut self, config: &NetworkConfig) {
        self.command.arg("-n").arg(format!(
            "tapif={},guest_mac={}",
            config.tap_device, config.mac_address
        ));
        self.command
            .arg("--vsock")
            .arg(config.vsock_cid.to_string());
    }
    fn setup_kernel(&mut self, config: &KernelConfig) {
        self.command.arg("-k").arg(&config.kernel_path);
        if let Some(initramfs_path) = &config.kernel_initramfs_path {
            self.command.arg("-i").arg(initramfs_path);
        }
        if let Some(kernel_cmd_params) = &config.kernel_cmd_params {
            self.command
                .arg("-p")
                .arg(&format!("\"{}\"", kernel_cmd_params));
        }
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
                .arg("-d")
                .arg(app_disk_path.to_string_lossy().to_string());
        }
    }
}

impl CommandRunner for LkvmRunner {
    fn get_command(&self) -> &Command {
        &self.command
    }
    fn setup_disk(&self, command: &mut Command, application_uuids: &[&Uuid]) {
        self.setup_disk(command, application_uuids);
    }
}
