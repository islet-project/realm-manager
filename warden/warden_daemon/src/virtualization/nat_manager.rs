use std::{collections::HashMap, net::IpAddr};

use async_trait::async_trait;
use fileter_table_handler::FilterIpTableManager;
use futures::TryStreamExt;
use ip_table_handler::{IpTableHandler, IpTableHandlerError};
use mangle_table_handler::MangleTableManager;
use nat_table_handler::NatIpTableManager;
use rtnetlink::{new_connection, Handle};
use tokio::task::block_in_place;
use tokio_tun::TunBuilder;
use uuid::Uuid;

use super::network_manager::{NetworkManager, NetworkManagerError};

mod fileter_table_handler;
mod ip_table_handler;
mod mangle_table_handler;
mod nat_table_handler;
mod utils;

pub struct NatManagerConfig {
    pub bridge_name: String,
    pub bridge_ip: IpAddr,
    pub bridge_mask: u8,
}

pub struct NatManager {
    taps: HashMap<Uuid, String>,
    handle: Handle,
    config: NatManagerConfig,
}

impl NatManager {
    pub async fn new(config: NatManagerConfig) -> Result<Self, NetworkManagerError> {
        let (connection, handle, _) = new_connection().unwrap();
        tokio::spawn(connection);
        let network_manager = Self {
            taps: HashMap::new(),
            handle,
            config,
        };
        network_manager.create_bridge().await?;
        network_manager.prepare_routing().await?;
        Ok(network_manager)
    }

    pub async fn shutdown(&mut self) -> Result<(), NetworkManagerError> {
        for tap_name in self.taps.values() {
            self.dalete_tap_device(tap_name.clone()).await?;
        }
        let bridge_id = self.get_device_id(self.config.bridge_name.clone()).await?;
        self.handle.link().del(bridge_id).execute().await.unwrap();
        self.cleanup_routing().await?;
        Ok(())
    }

    async fn create_bridge(&self) -> Result<(), NetworkManagerError> {
        self.handle
            .link()
            .add()
            .bridge(self.config.bridge_name.clone())
            .execute()
            .await
            .map_err(|err| NetworkManagerError::BridgeCreation {
                bridge_name: self.config.bridge_name.clone(),
                err_message: err.to_string(),
            })?;
        let bridge_id = self.get_device_id(self.config.bridge_name.clone()).await?;
        self.handle
            .address()
            .add(bridge_id, self.config.bridge_ip, self.config.bridge_mask)
            .execute()
            .await
            .map_err(|err| NetworkManagerError::BridgeIpAssign {
                bridge_name: self.config.bridge_name.clone(),
                err_message: err.to_string(),
            })?;
        self.handle
            .link()
            .set(bridge_id)
            .up()
            .execute()
            .await
            .map_err(|err| NetworkManagerError::BridgeUp {
                bridge_name: self.config.bridge_name.clone(),
                err_message: err.to_string(),
            })?;
        Ok(())
    }

    async fn add_tap_to_bridge(&self, tap_name: String) -> Result<(), NetworkManagerError> {
        let bridge_id = self.get_device_id(self.config.bridge_name.clone()).await?;
        let tap_id = self.get_device_id(tap_name.clone()).await?;
        self.handle
            .link()
            .set(tap_id)
            .controller(bridge_id)
            .execute()
            .await
            .map_err(|_| NetworkManagerError::BridgeAddIf {
                tap_name,
                bridge_name: self.config.bridge_name.clone(),
            })
    }

    async fn dalete_tap_device(&self, tap_name: String) -> Result<(), NetworkManagerError> {
        let tap_id = self.get_device_id(tap_name.clone()).await?;
        self.handle.link().del(tap_id).execute().await.map_err(|_| {
            NetworkManagerError::BridgeDelIf {
                tap_name,
                bridge_name: self.config.bridge_name.clone(),
            }
        })
    }

    async fn get_device_id(&self, device_name: String) -> Result<u32, NetworkManagerError> {
        self.handle
            .link()
            .get()
            .match_name(device_name.clone())
            .execute()
            .try_next()
            .await
            .map_err(|err| NetworkManagerError::NetLinkOperation(err.to_string()))?
            .ok_or(NetworkManagerError::MissingDevice(device_name))
            .map(|link| link.header.index)
    }

    async fn cleanup_routing(&self) -> Result<(), NetworkManagerError> {
        block_in_place(|| {
            self.cleanup_routing_sync()
                .map_err(|err| NetworkManagerError::IpTablesOperation(err.to_string()))
        })
    }

    fn cleanup_routing_sync(&self) -> Result<(), IpTableHandlerError> {
        NatIpTableManager::new(self.config.bridge_ip, self.config.bridge_mask)?
            .remove_ip_table_rules()?;
        FilterIpTableManager::new(
            self.config.bridge_name.clone(),
            self.config.bridge_ip,
            self.config.bridge_mask,
        )?
        .remove_ip_table_rules()?;
        MangleTableManager::new(self.config.bridge_name.clone(), self.config.bridge_ip)?
            .remove_ip_table_rules()
    }

    async fn prepare_routing(&self) -> Result<(), NetworkManagerError> {
        block_in_place(|| {
            self.prepare_routing_sync()
                .map_err(|err| NetworkManagerError::IpTablesOperation(err.to_string()))
        })
    }

    fn prepare_routing_sync(&self) -> Result<(), IpTableHandlerError> {
        NatIpTableManager::new(self.config.bridge_ip, self.config.bridge_mask)?
            .insert_ip_table_rules()?;
        FilterIpTableManager::new(
            self.config.bridge_name.clone(),
            self.config.bridge_ip,
            self.config.bridge_mask,
        )?
        .insert_ip_table_rules()?;
        MangleTableManager::new(self.config.bridge_name.clone(), self.config.bridge_ip)?
            .insert_ip_table_rules()
    }
}

#[async_trait]
impl NetworkManager for NatManager {
    async fn create_tap_device_for_realm(
        &mut self,
        tap_name: String,
        realm_id: Uuid,
    ) -> Result<(), NetworkManagerError> {
        if self.taps.contains_key(&realm_id) {
            return Err(NetworkManagerError::TapCreation {
                tap_name,
                err_message: "already exists.".to_string(),
            });
        }
        TunBuilder::new()
            .name(&tap_name)
            .persist()
            .up()
            .tap(true)
            .try_build()
            .map_err(|err| NetworkManagerError::TapCreation {
                tap_name: tap_name.clone(),
                err_message: err.to_string(),
            })?;

        self.add_tap_to_bridge(tap_name.clone()).await?;
        self.taps.insert(realm_id, tap_name);
        Ok(())
    }

    async fn shutdown_tap_device_for_realm(
        &mut self,
        realm_id: Uuid,
    ) -> Result<(), NetworkManagerError> {
        let tap_name = self
            .taps
            .get(&realm_id)
            .ok_or(NetworkManagerError::MissingTap {
                realm_uuid: realm_id,
            })?;

        self.dalete_tap_device(tap_name.clone()).await?;
        self.taps.remove(&realm_id);
        Ok(())
    }
}
