use std::collections::HashMap;

use super::network::{NatConfig, NetworkManager, NetworkManagerError};
use async_trait::async_trait;
use bridge_handler::VirtualBridgeHandler;
use devices::{Bridge, Tap};
use fileter_table_handler::FilterIpTableManager;
use ip_table_handler::{IpTableHandler, IpTableHandlerError};
use mangle_table_handler::MangleTableManager;
use nat_table_handler::NatIpTableManager;
use tap_handler::TapDeviceFabric;
use tokio::task::block_in_place;
use uuid::Uuid;

mod bridge_handler;
mod devices;
mod fileter_table_handler;
mod ip_table_handler;
mod mangle_table_handler;
mod nat_table_handler;
mod tap_handler;
mod utils;

pub struct NetworkManagerHandler {
    config: NatConfig,
    bridge: Box<dyn Bridge + Send + Sync>,
    taps: HashMap<Uuid, Box<dyn Tap + Send + Sync>>,
}

impl NetworkManagerHandler {
    fn cleanup_routing(config: NatConfig) -> Result<(), NetworkManagerError> {
        block_in_place(|| {
            Self::cleanup_routing_sync(config)
                .map_err(|err| NetworkManagerError::DestroyNatNetwork(err.to_string()))
        })
    }

    fn cleanup_routing_sync(config: NatConfig) -> Result<(), IpTableHandlerError> {
        NatIpTableManager::new(config.net_if_ip, config.net_if_mask)?.remove_ip_table_rules()?;
        FilterIpTableManager::new(
            config.net_if_name.clone(),
            config.net_if_ip,
            config.net_if_mask,
        )?
        .remove_ip_table_rules()?;
        MangleTableManager::new(config.net_if_name.clone(), config.net_if_ip)?
            .remove_ip_table_rules()
    }

    fn prepare_routing(config: NatConfig) -> Result<(), NetworkManagerError> {
        block_in_place(|| {
            Self::prepare_routing_sync(config)
                .map_err(|err| NetworkManagerError::CreateNatNetwork(err.to_string()))
        })
    }

    fn prepare_routing_sync(config: NatConfig) -> Result<(), IpTableHandlerError> {
        NatIpTableManager::new(config.net_if_ip, config.net_if_mask)?.insert_ip_table_rules()?;
        FilterIpTableManager::new(
            config.net_if_name.clone(),
            config.net_if_ip,
            config.net_if_mask,
        )?
        .insert_ip_table_rules()?;
        MangleTableManager::new(config.net_if_name.clone(), config.net_if_ip)?
            .insert_ip_table_rules()
    }

    async fn shutdown_all_taps(&mut self) -> Result<(), NetworkManagerError> {
        let ids: Vec<Uuid> = self.taps.keys().copied().collect();
        for id in ids {
            self.shutdown_tap_device_for_realm(id).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl NetworkManager for NetworkManagerHandler {
    async fn create_nat(config: NatConfig) -> Result<Self, NetworkManagerError> {
        let bridge = VirtualBridgeHandler::create_bridge(
            config.net_if_name.clone(),
            config.net_if_ip,
            config.net_if_mask,
        )
        .await
        .map_err(|err| NetworkManagerError::CreateNatNetwork(err.to_string()))?;
        Self::prepare_routing(config.clone())
            .map_err(|err| NetworkManagerError::CreateNatNetwork(err.to_string()))?;
        Ok(Self {
            config,
            bridge,
            taps: HashMap::new(),
        })
    }
    async fn shutdown_nat(&mut self) -> Result<(), NetworkManagerError> {
        self.shutdown_all_taps().await?;
        VirtualBridgeHandler::remove_ip_table_rules(&self.bridge)
            .await
            .map_err(|err| NetworkManagerError::DestroyNatNetwork(err.to_string()))?;
        Self::cleanup_routing(self.config.clone())?;
        Ok(())
    }

    async fn create_tap_device_for_realm(
        &mut self,
        name: String,
        realm_id: Uuid,
    ) -> Result<(), NetworkManagerError> {
        let tap = TapDeviceFabric::create_tap(name)
            .await
            .map_err(|err| NetworkManagerError::CreateTapDevice(err.to_string()))?;
        self.bridge
            .add_tap_device_to_bridge(&tap)
            .await
            .map_err(|err| NetworkManagerError::CreateTapDevice(err.to_string()))?;
        self.taps.insert(realm_id, tap);
        Ok(())
    }
    async fn shutdown_tap_device_for_realm(
        &mut self,
        realm_id: Uuid,
    ) -> Result<(), NetworkManagerError> {
        let tap = self
            .taps
            .remove(&realm_id)
            .ok_or(NetworkManagerError::DestroyTapDevice(format!(
                "No tap device for realm: {}",
                realm_id
            )))?;
        self.bridge
            .remove_tap_device_from_bridge(&tap)
            .await
            .map_err(|err| NetworkManagerError::DestroyTapDevice(err.to_string()))?;
        TapDeviceFabric::delete_tap(tap)
            .await
            .map_err(|err| NetworkManagerError::DestroyTapDevice(err.to_string()))
    }
}
