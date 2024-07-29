use std::str::FromStr;

use log::info;
use uuid::Uuid;
use warden_client::{
    applciation::ApplicationConfig,
    realm::{CpuConfig, KernelConfig, MemoryConfig, NetworkConfig, RealmConfig},
};
use warden_client_lib::WardenConnection;

use crate::commands::Command;

pub struct CommandHanlder {
    connection: WardenConnection,
}

impl CommandHanlder {
    pub fn new(connection: WardenConnection) -> Self {
        Self { connection }
    }

    pub async fn handle_command(&mut self, command: Command) -> Result<(), anyhow::Error> {
        match command {
            Command::CreateRealm {
                cpu,
                machine,
                core_count,
                ram_size,
                tap_device,
                network_device,
                remote_terminal_uri,
                mac_address,
                vsock_cid,
                kernel,
            } => {
                let cpu = CpuConfig {
                    cpu,
                    cores_number: core_count,
                };
                let kernel = KernelConfig {
                    kernel_path: kernel,
                };
                let memory = MemoryConfig { ram_size };
                let network = NetworkConfig {
                    vsock_cid,
                    tap_device,
                    mac_address,
                    hardware_device: network_device,
                    remote_terminal_uri,
                };
                let realm_config = RealmConfig {
                    machine,
                    cpu,
                    memory,
                    network,
                    kernel,
                };

                let realm_uuid = self.connection.create_realm(realm_config).await?;
                info!("Created realm with uuid: {realm_uuid}");
                Ok(())
            }
            Command::StartRealm { id } => {
                Ok(self.connection.start_realm(Uuid::from_str(&id)?).await?)
            }
            Command::StopRealm { id } => {
                Ok(self.connection.stop_realm(Uuid::from_str(&id)?).await?)
            }
            Command::InspectRealm { id: realm_id } => {
                let realm_data = self
                    .connection
                    .inspect_realm(Uuid::from_str(&realm_id)?)
                    .await?;
                info!("Realm data: {realm_data:#?}");
                Ok(())
            }
            Command::ListRealms {} => {
                let realms_data = self.connection.list_realms().await?;
                info!("Realms data: {realms_data:#?}");
                Ok(())
            }
            Command::DestroyRealm { id } => {
                Ok(self.connection.destroy_realm(Uuid::from_str(&id)?).await?)
            }
            Command::RebootRealm { id } => {
                Ok(self.connection.reboot_realm(Uuid::from_str(&id)?).await?)
            }
            Command::CreateApplication { realm_id } => {
                let application_config = ApplicationConfig {};
                let application_uuid = self
                    .connection
                    .create_application(Uuid::from_str(&realm_id)?, application_config)
                    .await?;
                info!("Application uuid: {application_uuid}");
                Ok(())
            }
            Command::StartApplication {
                application_id,
                realm_id,
            } => Ok(self
                .connection
                .start_application(Uuid::from_str(&realm_id)?, Uuid::from_str(&application_id)?)
                .await?),
            Command::StopApplication {
                application_id,
                realm_id,
            } => Ok(self
                .connection
                .stop_application(Uuid::from_str(&realm_id)?, Uuid::from_str(&application_id)?)
                .await?),
            Command::UpdateApplication {
                application_id,
                realm_id,
            } => {
                let application_config = ApplicationConfig {};
                self.connection
                    .update_application(
                        Uuid::from_str(&realm_id)?,
                        Uuid::from_str(&application_id)?,
                        application_config,
                    )
                    .await?;
                Ok(())
            }
        }
    }
}