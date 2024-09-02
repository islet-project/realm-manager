use std::{
    env,
    net::Ipv4Addr,
    path::{Path, PathBuf},
    str::FromStr,
};

use ipnet::{IpNet, Ipv4Net};
use nix::{
    sys::signal::{self, Signal::SIGINT},
    unistd::Pid,
};
use tempfile::{tempdir, TempDir};
use tokio_vsock::VMADDR_CID_HOST;
use warden_client::realm::RealmConfig;
use warden_client::realm::{CpuConfig, KernelConfig, MemoryConfig, NetworkConfig};
use warden_daemon::cli::Cli;

pub struct WorkdirManager {
    temp_dir: TempDir,
}

impl WorkdirManager {
    pub async fn new() -> Self {
        let temp_dir = tempdir().expect("Can't create temporary dir.");
        Self { temp_dir }
    }

    pub fn get_path(&self) -> &Path {
        self.temp_dir.path()
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
    const STARTUP_TIMEOUT_ENV: &str = "REALM_STARTUP_TIMEOUT";
    const NAT_NETWORK_NAME_ENV: &str = "NAT_NETWORK_NAME";
    const NAT_NETWORK_IP_ENV: &str = "NAT_NETWORK_IP";
    const DHCP_BINARY_PATH_ENV: &str = "DHCP_EXEC_PATH";
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
        realm_connection_wait_time_secs: env::var(STARTUP_TIMEOUT_ENV)
            .map(|timeout_sec| u64::from_str(&timeout_sec).unwrap())
            .unwrap_or(60),
        bridge_name: env::var(NAT_NETWORK_NAME_ENV).unwrap_or("virtbDaemonTest".to_string()),
        bridge_ip: env::var(NAT_NETWORK_IP_ENV)
            .map(|ip_str| IpNet::from_str(&ip_str).unwrap())
            .unwrap_or(IpNet::V4(
                Ipv4Net::new(Ipv4Addr::new(192, 168, 100, 0), 24).unwrap(),
            )),
        dhcp_exec_path: PathBuf::from_str(
            &env::var(DHCP_BINARY_PATH_ENV).expect("Missing path to DHCP server binary!"),
        )
        .unwrap(),
    }
}
