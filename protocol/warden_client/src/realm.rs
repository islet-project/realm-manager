use serde::{Deserialize, Serialize};
use std::{net::IpAddr, path::PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum State {
    Halted,
    Provisioning,
    Running,
    NeedReboot,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct RealmNetwork {
    pub ip: IpAddr,
    pub if_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct RealmDescription {
    pub uuid: Uuid,
    pub state: State,
    pub applications: Vec<Uuid>,
    pub network: Vec<RealmNetwork>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct RealmConfig {
    pub machine: String,
    pub cpu: CpuConfig,
    pub memory: MemoryConfig,
    pub network: NetworkConfig,
    pub kernel: KernelConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct CpuConfig {
    pub cpu: String,
    pub cores_number: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct MemoryConfig {
    pub ram_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct NetworkConfig {
    pub vsock_cid: u32,
    pub tap_device: String,
    pub mac_address: String,
    pub hardware_device: Option<String>,
    pub remote_terminal_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct KernelConfig {
    pub kernel_path: PathBuf,
    pub kernel_initramfs_path: Option<PathBuf>,
    pub kernel_cmd_params: Option<String>,
}
