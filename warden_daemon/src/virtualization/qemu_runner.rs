use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use crate::managers::realm_configuration::{
    CpuConfig, DiscConfig, KernelConfig, MemoryConfig, NetworkConfig,
};
use crate::managers::realm_manager::{VmManager, VmManagerError};

pub struct QemuRunner {
    command: Command,
    vm: Option<Child>,
}

impl QemuRunner {
    pub fn new(path_to_qemu: PathBuf) -> Self {
        QemuRunner {
            command: Command::new(path_to_qemu),
            vm: None,
        }
    }
}

impl VmManager for QemuRunner {
    fn setup_network(&mut self, config: &NetworkConfig) {
        self.command.arg("-netdev").arg(format!(
            "tap,id=mynet0,ifname={},script=no,downscript=no",
            &config.tap_device
        ));
        self.command.arg("-device").arg(format!(
            "{},netdev=mynet0",
            config
                .hardware_device
                .as_ref()
                .get_or_insert(&String::from("e1000"))
        ));
        self.command.arg("-device").arg(format!(
            "vhost-vsock-pci,id=vhost-vsock-pci0,guest-cid={}",
            config.vsock_cid
        ));
    }
    fn setup_cpu(&mut self, config: &CpuConfig) {
        self.command
            .arg("-smp")
            .arg(config.cores_number.to_string());
        self.command.arg("-cpu").arg(&config.cpu);
    }
    fn setup_disc(&mut self, config: &DiscConfig) {
        if let Some(drive) = &config.drive {
            if let Some(format) = &config.drive_format {
                self.command
                    .arg("-drive")
                    .arg(format!("file={},format={}", drive, format));
            }
        }
    }
    fn setup_memory(&mut self, config: &MemoryConfig) {
        self.command.arg("-m").arg(config.ram_size.to_string());
    }
    fn setup_machine(&mut self, name: &String) {
        self.command.arg("-machine").arg(&name);
    }
    fn launch_vm(&mut self, config: &KernelConfig) -> Result<(), VmManagerError> {
        self.command.arg("-enable-kvm");
        self.command.arg("-cdrom").arg(&config.kernel_path);

        self.command.stdin(Stdio::null());
        self.command.stdout(Stdio::piped());
        self.command.stderr(Stdio::piped());

        match self.command.spawn() {
            Ok(vm) => {
                self.vm = Some(vm);
                Ok(())
            }
            Err(err) => Err(VmManagerError::LaunchFail(err)),
        }
    }
    fn stop_vm(&mut self) -> Result<(), VmManagerError> {
        match &mut self.vm {
            Some(child) => child.kill().map_err(|_| VmManagerError::StopFail),
            None => Err(VmManagerError::VmNotLaunched),
        }
    }
    fn delete_vm(&self) {
        todo!()
    }
}