use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct RealmConfig {
    pub machine: String,
    pub cpu: CpuConfig,
    pub memory: MemoryConfig,
    pub network: NetworkConfig,
    pub disc: DiscConfig,
    pub kernel: KernelConfig,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CpuConfig {
    pub cpu: String,
    pub cores_number: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MemoryConfig {
    pub ram_size: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NetworkConfig {
    pub vsock_cid: u32,
    pub tap_device: String,
    pub hardware_device: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DiscConfig {
    pub drive: Option<String>,
    pub drive_format: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct KernelConfig {
    pub kernel_path: String,
}
