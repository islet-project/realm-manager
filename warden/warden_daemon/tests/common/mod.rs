use std::{
    env,
    fs::{remove_dir_all, remove_file},
    path::{Path, PathBuf},
    str::FromStr,
};

use nix::{
    sys::signal::{self, Signal::SIGINT},
    unistd::Pid,
};
use tokio_vsock::VMADDR_CID_HOST;
use uuid::Uuid;
use warden_client::realm::RealmConfig;
use warden_client::realm::{CpuConfig, KernelConfig, MemoryConfig, NetworkConfig};
use warden_daemon::cli::Cli;

pub struct PathResourceManager {
    resource_path: Box<dyn AsRef<Path>>,
}

impl PathResourceManager {
    pub async fn new() -> Self {
        const TEST_FOLDER_PATH: &str = "/tmp/warden-daemon-integration-tests";
        tokio::fs::create_dir_all(TEST_FOLDER_PATH).await.unwrap();
        Self {
            resource_path: Box::new(format!("{}/{}", TEST_FOLDER_PATH, Uuid::new_v4())),
        }
    }

    pub fn get_path(&self) -> &Path {
        (*self.resource_path).as_ref()
    }
}

impl Drop for PathResourceManager {
    fn drop(&mut self) {
        let path = self.get_path();
        if let Err(_) = remove_dir_all(path) {
            let _ = remove_file(path);
        }
    }
}

#[allow(dead_code)]
pub fn request_shutdown() {
    signal::kill(Pid::this(), SIGINT).unwrap();
}

#[allow(dead_code)]
pub fn get_kernel_path() -> PathBuf {
    const REALM_KERNEL_PATH_ENV: &str = "REALM_KERNEL_PATH";
    PathBuf::from_str(
        &env::var(REALM_KERNEL_PATH_ENV)
            .expect(&format!("Missing env var: {}", REALM_KERNEL_PATH_ENV)),
    )
    .unwrap()
}

#[allow(dead_code)]
pub fn create_example_realm_config() -> RealmConfig {
    const TAP_DEVICE_ENV: &str = "TAP_DEVICE";
    RealmConfig {
        machine: "virt".to_string(),
        cpu: CpuConfig {
            cpu: "cortex-a57".to_string(),
            cores_number: 2,
        },
        memory: MemoryConfig { ram_size: 2048 },
        network: NetworkConfig {
            vsock_cid: 12344,
            tap_device: env::var(TAP_DEVICE_ENV).unwrap_or("tap100".to_string()),
            mac_address: "52:55:00:d1:55:01".to_string(),
            hardware_device: Some("e1000".to_string()),
            remote_terminal_uri: None,
        },
        kernel: KernelConfig {
            kernel_path: get_kernel_path(),
        },
    }
}

pub fn create_example_cli(unix_sock_path: PathBuf, warden_workdir_path: PathBuf) -> Cli {
    const REALM_QEMU_PATH_ENV: &str = "REALM_QEMU_PATH";
    const WARDEN_VSOCK_PORT_ENV: &str = "WARDEN_VSOCK_PORT";
    Cli {
        cid: VMADDR_CID_HOST,
        port: env::var(WARDEN_VSOCK_PORT_ENV)
            .map(|port| u32::from_str(&port).unwrap())
            .unwrap_or(1337),
        qemu_path: PathBuf::from_str(
            &env::var(REALM_QEMU_PATH_ENV)
                .expect(&format!("Missing env var: {}", REALM_QEMU_PATH_ENV)),
        )
        .unwrap(),
        unix_sock_path,
        warden_workdir_path,
        realm_connection_wait_time_secs: 60,
    }
}
