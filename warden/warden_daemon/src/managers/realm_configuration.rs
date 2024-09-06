use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct RealmConfig {
    pub machine: String,
    pub cpu: CpuConfig,
    pub memory: MemoryConfig,
    pub network: NetworkConfig,
    pub kernel: KernelConfig,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct CpuConfig {
    pub cpu: String,
    pub cores_number: usize,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct MemoryConfig {
    pub ram_size: usize,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct NetworkConfig {
    pub vsock_cid: u32,
    pub tap_device: String,
    pub mac_address: String,
    pub hardware_device: Option<String>,
    pub remote_terminal_uri: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct KernelConfig {
    pub kernel_path: PathBuf,
    pub kernel_initramfs_path: Option<PathBuf>,
    pub kernel_cmd_params: Option<String>,
}
