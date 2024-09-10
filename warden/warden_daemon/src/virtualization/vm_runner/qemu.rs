use std::path::PathBuf;
use std::process::{Command, Stdio};
use uuid::Uuid;

use crate::managers::realm_configuration::{
    CpuConfig, KernelConfig, MemoryConfig, NetworkConfig, RealmConfig,
};
use crate::storage::app_disk_manager::ApplicationDiskManager;

use super::command_runner::CommandRunner;

pub struct QemuRunner {
    realm_workdir: PathBuf,
    command: Command,
}

impl QemuRunner {
    pub fn new(path_to_qemu: PathBuf, realm_workdir: PathBuf, config: &RealmConfig) -> Self {
        let mut runner = QemuRunner {
            realm_workdir,
            command: Command::new(path_to_qemu),
        };
        runner.setup_cpu(&config.cpu);
        runner.setup_kernel(&config.kernel);
        runner.setup_memory(&config.memory);
        runner.setup_machine(&config.machine);
        runner.setup_network(&config.network);
        runner.control_output();
        runner
    }
    fn setup_network(&mut self, config: &NetworkConfig) {
        self.command.arg("-netdev").arg(format!(
            "tap,id=mynet0,ifname={},script=no,downscript=no",
            &config.tap_device
        ));
        self.command.arg("-device").arg(format!(
            "{},netdev=mynet0,mac={}",
            config
                .hardware_device
                .as_ref()
                .get_or_insert(&String::from("e1000")),
            config.mac_address
        ));
        self.command.arg("-device").arg(format!(
            "vhost-vsock-pci,id=vhost-vsock-pci0,guest-cid={}",
            config.vsock_cid
        ));
        if let Some(terminal_uri) = &config.remote_terminal_uri {
            // Setup access terminal
            self.command.arg("-serial").arg(terminal_uri);
        }
    }
    fn setup_kernel(&mut self, config: &KernelConfig) {
        self.command.arg("-kernel").arg(&config.kernel_path);
    }
    fn setup_cpu(&mut self, config: &CpuConfig) {
        self.command
            .arg("-smp")
            .arg(config.cores_number.to_string());
        self.command.arg("-cpu").arg(&config.cpu);
    }
    fn setup_memory(&mut self, config: &MemoryConfig) {
        self.command.arg("-m").arg(config.ram_size.to_string());
    }
    fn setup_machine(&mut self, name: &str) {
        self.command.arg("-machine").arg(name);
    }
    fn control_output(&mut self) {
        self.command.arg("-nographic");
        self.command.arg("-append").arg("console=ttyAMA0");

        self.command.stdin(Stdio::null());
        self.command.stdout(Stdio::piped());
        self.command.stderr(Stdio::piped());
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
}

impl CommandRunner for QemuRunner {
    fn get_command(&self) -> &Command {
        &self.command
    }
    fn setup_disk(&self, command: &mut Command, application_uuids: &[&Uuid]) {
        self.setup_disk(command, application_uuids);
    }
}
