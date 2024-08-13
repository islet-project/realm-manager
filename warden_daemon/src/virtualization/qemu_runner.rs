use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};

use log::trace;

use crate::managers::realm_configuration::{CpuConfig, KernelConfig, MemoryConfig, NetworkConfig};
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
    fn launch_vm(&mut self) -> Result<(), VmManagerError> {
        self.control_output();
        trace!("Spawning realm with command: {:?}", self.command);
        self.command
            .spawn()
            .map(|child| {
                self.vm = Some(child);
            })
            .map_err(VmManagerError::LaunchFail)
    }
    fn stop_vm(&mut self) -> Result<(), VmManagerError> {
        self.vm
            .as_mut()
            .map(|child| child.kill().map_err(|_| VmManagerError::StopFail))
            .unwrap_or(Err(VmManagerError::VmNotLaunched))
    }
    fn delete_vm(&mut self) -> Result<(), VmManagerError> {
        self.stop_vm()
            .map_err(|err| VmManagerError::DestroyFail(err.to_string()))?;
        self.get_exit_status();
        Ok(())
    }
    fn get_exit_status(&mut self) -> Option<ExitStatus> {
        if let Some(vm) = &mut self.vm {
            return vm.try_wait().ok()?;
        }
        None
    }
}

impl QemuRunner {
    fn control_output(&mut self) {
        self.command.arg("-nographic");
        self.command.arg("-append").arg("console=ttyAMA0");

        self.command.stdin(Stdio::null());
        self.command.stdout(Stdio::piped());
        self.command.stderr(Stdio::piped());
    }
}
