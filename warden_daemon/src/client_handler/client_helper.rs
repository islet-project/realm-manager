use crate::managers::{
    application::ApplicationConfig,
    realm::{RealmData, RealmDescription, State},
    realm_configuration::{CpuConfig, KernelConfig, MemoryConfig, NetworkConfig, RealmConfig},
};

use super::client_command_handler::ClientError;

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

impl From<warden_client::realm::RealmDescription> for RealmDescription {
    fn from(value: warden_client::realm::RealmDescription) -> Self {
        Self {
            uuid: value.uuid,
            realm_data: RealmData {
                state: value.state.into(),
            },
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
        }
    }
}

impl From<warden_client::applciation::ApplicationConfig> for ApplicationConfig {
    fn from(_value: warden_client::applciation::ApplicationConfig) -> Self {
        Self {}
    }
}

impl From<ClientError> for warden_client::client::WardenDaemonError {
    fn from(val: ClientError) -> Self {
        type WardenDaemonError = warden_client::client::WardenDaemonError;
        match val {
            ClientError::ReadingRequestFail => WardenDaemonError::ReadingRequestFail,
            ClientError::UnknownCommand { length: _ } => WardenDaemonError::UnknownCommand,
            ClientError::SendingResponseFail => WardenDaemonError::SendingResponseFail,
            err => WardenDaemonError::WardenDaemonFail {
                message: err.to_string(),
            },
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

impl From<RealmDescription> for warden_client::realm::RealmDescription {
    fn from(val: RealmDescription) -> Self {
        type RealmDescription = warden_client::realm::RealmDescription;
        RealmDescription {
            uuid: val.uuid,
            state: val.realm_data.state.into(),
        }
    }
}
