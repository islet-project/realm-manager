use warden_realm::ApplicationInfo;

use crate::managers::{
    application::{ApplicationConfig, ApplicationData},
    realm::{RealmDescription, RealmNetwork, State},
    realm_client::RealmProvisioningConfig,
    realm_configuration::{CpuConfig, KernelConfig, MemoryConfig, NetworkConfig, RealmConfig},
};

impl From<warden_client::realm::RealmConfig> for RealmConfig {
    fn from(realm_config: warden_client::realm::RealmConfig) -> Self {
        RealmConfig {
            machine: realm_config.machine,
            cpu: realm_config.cpu.into(),
            memory: realm_config.memory.into(),
            network: realm_config.network.into(),
            kernel: realm_config.kernel.into(),
        }
    }
}

impl From<warden_client::realm::State> for State {
    fn from(state: warden_client::realm::State) -> Self {
        type CState = warden_client::realm::State;
        match state {
            CState::Halted => Self::Halted,
            CState::Provisioning => Self::Provisioning,
            CState::Running => Self::Running,
            CState::NeedReboot => Self::NeedReboot,
        }
    }
}

impl From<warden_client::realm::CpuConfig> for CpuConfig {
    fn from(value: warden_client::realm::CpuConfig) -> Self {
        Self {
            cpu: value.cpu,
            cores_number: value.cores_number,
        }
    }
}

impl From<warden_client::realm::MemoryConfig> for MemoryConfig {
    fn from(value: warden_client::realm::MemoryConfig) -> Self {
        Self {
            ram_size: value.ram_size,
        }
    }
}

impl From<warden_client::realm::NetworkConfig> for NetworkConfig {
    fn from(value: warden_client::realm::NetworkConfig) -> Self {
        Self {
            vsock_cid: value.vsock_cid,
            tap_device: value.tap_device,
            mac_address: value.mac_address,
            hardware_device: value.hardware_device,
            remote_terminal_uri: value.remote_terminal_uri,
        }
    }
}

impl From<warden_client::realm::KernelConfig> for KernelConfig {
    fn from(value: warden_client::realm::KernelConfig) -> Self {
        Self {
            kernel_path: value.kernel_path,
            kernel_initramfs_path: None,
            kernel_cmd_params: None,
        }
    }
}

impl From<warden_client::application::ApplicationConfig> for ApplicationConfig {
    fn from(value: warden_client::application::ApplicationConfig) -> Self {
        Self {
            name: value.name,
            version: value.version,
            image_registry: value.image_registry,
            image_storage_size_mb: value.image_storage_size_mb,
            data_storage_size_mb: value.data_storage_size_mb,
        }
    }
}

impl From<State> for warden_client::realm::State {
    fn from(val: State) -> Self {
        type CState = warden_client::realm::State;
        match val {
            State::Halted => CState::Halted,
            State::Provisioning => CState::Provisioning,
            State::Running => CState::Running,
            State::NeedReboot => CState::NeedReboot,
        }
    }
}

impl From<RealmNetwork> for warden_client::realm::RealmNetwork {
    fn from(value: RealmNetwork) -> Self {
        Self {
            ip: value.ip,
            if_name: value.if_name,
        }
    }
}

impl From<RealmDescription> for warden_client::realm::RealmDescription {
    fn from(val: RealmDescription) -> Self {
        type RealmDescription = warden_client::realm::RealmDescription;
        RealmDescription {
            uuid: val.uuid,
            state: val.realm_data.state.into(),
            applications: val.realm_data.applications,
            network: val
                .realm_data
                .ips
                .into_iter()
                .map(|data| data.into())
                .collect(),
        }
    }
}

impl From<ApplicationData> for ApplicationInfo {
    fn from(value: ApplicationData) -> Self {
        ApplicationInfo {
            id: value.id,
            name: value.name,
            version: value.version,
            image_registry: value.image_registry,
            image_part_uuid: value.image_part_uuid,
            data_part_uuid: value.data_part_uuid,
        }
    }
}

impl From<RealmProvisioningConfig> for Vec<ApplicationInfo> {
    fn from(val: RealmProvisioningConfig) -> Self {
        val.applications_data
            .into_iter()
            .map(|data| data.into())
            .collect()
    }
}
