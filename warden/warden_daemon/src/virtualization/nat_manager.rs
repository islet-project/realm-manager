use std::{collections::HashMap, net::IpAddr};

use super::{
    dhcp::{DHCPError, DHCPServer},
    network::{NetworkConfig, NetworkManager, NetworkManagerError},
};
use async_trait::async_trait;
use bridge_handler::VirtualBridgeHandler;
use devices::{Bridge, Tap};
use filter_table_handler::FilterIpTableManager;
use ip_table_handler::{IpTableHandler, IpTableHandlerError};
use ipnet::{IpAdd, IpNet};
use log::{error, info};
use mangle_table_handler::MangleTableManager;
use nat_table_handler::NatIpTableManager;
use tap_handler::TapDeviceFabric;
use tokio::task::block_in_place;
use uuid::Uuid;

mod bridge_handler;
mod devices;
mod filter_table_handler;
mod ip_table_handler;
mod mangle_table_handler;
mod nat_table_handler;
mod tap_handler;
mod utils;

pub struct NetworkManagerHandler<DHCP: DHCPServer + Send + Sync> {
    config: NetworkConfig,
    bridge: Box<dyn Bridge + Send + Sync>,
    dhcp_server: DHCP,
    taps: HashMap<Uuid, Box<dyn Tap + Send + Sync>>,
}

impl<DHCP: DHCPServer + Send + Sync> NetworkManagerHandler<DHCP> {
    fn cleanup_routing(config: NetworkConfig) -> Result<(), NetworkManagerError> {
        block_in_place(|| {
            Self::cleanup_routing_sync(config)
                .map_err(|err| NetworkManagerError::DestroyNatNetwork(err.to_string()))
        })
    }

    fn cleanup_routing_sync(config: NetworkConfig) -> Result<(), IpTableHandlerError> {
        NatIpTableManager::new(config.net_if_name.clone(), config.net_if_ip)?
            .remove_ip_table_rules()?;
        FilterIpTableManager::new(config.net_if_name.clone(), config.net_if_ip)?
            .remove_ip_table_rules()?;
        MangleTableManager::new(config.net_if_name.clone(), config.net_if_ip)?
            .remove_ip_table_rules()
    }

    fn prepare_routing(config: NetworkConfig) -> Result<(), NetworkManagerError> {
        block_in_place(|| {
            Self::prepare_routing_sync(config)
                .map_err(|err| NetworkManagerError::CreateNatNetwork(err.to_string()))
        })
    }

    fn prepare_routing_sync(config: NetworkConfig) -> Result<(), IpTableHandlerError> {
        NatIpTableManager::new(config.net_if_name.clone(), config.net_if_ip)?
            .insert_ip_table_rules()?;
        FilterIpTableManager::new(config.net_if_name.clone(), config.net_if_ip)?
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

    fn calculate_bridge_addr(hostnet_ip: &IpNet) -> Result<IpNet, NetworkManagerError> {
        IpNet::new(
            match hostnet_ip.addr() {
                std::net::IpAddr::V4(ip) => IpAddr::V4(ip.saturating_add(1)),
                std::net::IpAddr::V6(ip) => IpAddr::V6(ip.saturating_add(1)),
            },
            hostnet_ip.prefix_len(),
        )
        .map_err(|err| {
            NetworkManagerError::CreateNatNetwork(format!("Failed to compute bridge addr: {}", err))
        })
    }

    async fn setup_routing(
        config: &NetworkConfig,
        bridge: &(dyn Bridge + Send + Sync),
    ) -> Result<(), NetworkManagerError> {
        match Self::prepare_routing(config.clone()) {
            Err(err) => {
                Self::destroy_bridge(bridge).await?;
                Err(err)
            }
            _ => Ok(()),
        }
    }

    async fn destroy_bridge(
        bridge: &(dyn Bridge + Send + Sync),
    ) -> Result<(), NetworkManagerError> {
        VirtualBridgeHandler::delete_bridge(bridge)
            .await
            .map_err(|err| NetworkManagerError::DestroyNatNetwork(err.to_string()))
    }

    async fn handle_dhcp_serve_start(
        result: Result<(), DHCPError>,
        bridge: &(dyn Bridge + Send + Sync),
        config: &NetworkConfig,
    ) -> Result<(), NetworkManagerError> {
        match result {
            Err(err) => {
                Self::destroy_bridge(bridge).await?;
                Self::cleanup_routing(config.clone())?;
                Err(NetworkManagerError::CreateNatNetwork(err.to_string()))
            }
            _ => Ok(()),
        }
    }
}

#[async_trait]
impl<DHCP: DHCPServer + Send + Sync> NetworkManager for NetworkManagerHandler<DHCP> {
    type DHCPServer = DHCP;
    async fn create_nat(
        config: NetworkConfig,
        mut dhcp_server: Self::DHCPServer,
    ) -> Result<Self, NetworkManagerError> {
        let bridge_ip = Self::calculate_bridge_addr(&config.net_if_ip)?;

        info!("Creating Bridge: {}", &config.net_if_name);
        let bridge = VirtualBridgeHandler::create_bridge(config.net_if_name.clone(), bridge_ip)
            .await
            .map_err(|err| NetworkManagerError::CreateNatNetwork(err.to_string()))?;
        info!("Bridge: {} created!", &config.net_if_name);

        info!("Seting up routing ...");
        Self::setup_routing(&config, bridge.as_ref()).await?;
        info!("Setting up routing finished!");

        info!("Starting DHCP server ...");
        Self::handle_dhcp_serve_start(
            dhcp_server.start(bridge_ip, &config.net_if_name).await,
            bridge.as_ref(),
            &config,
        )
        .await?;
        info!("DHCP server started!");

        Ok(Self {
            config,
            bridge,
            dhcp_server,
            taps: HashMap::new(),
        })
    }
    async fn shutdown_nat(&mut self) {
        if let Err(err) = self.shutdown_all_taps().await {
            error!("{}", err);
        }
        if let Err(err) = self.dhcp_server.stop().await {
            error!(
                "{}",
                NetworkManagerError::DestroyNatNetwork(err.to_string())
            );
        }
        if let Err(err) = Self::destroy_bridge(self.bridge.as_ref()).await {
            error!("{}", err);
        }
        if let Err(err) = Self::cleanup_routing(self.config.clone()) {
            error!("{}", err);
        }
    }

    async fn create_tap_device_for_realm(
        &mut self,
        name: String,
        realm_id: Uuid,
    ) -> Result<(), NetworkManagerError> {
        info!("Creating tap device: {} for realm: {}", &name, &realm_id);
        let tap = TapDeviceFabric::create_tap(name)
            .await
            .map_err(|err| NetworkManagerError::CreateTapDevice(err.to_string()))?;
        self.bridge
            .add_tap_device_to_bridge(tap.as_ref())
            .await
            .map_err(|err| NetworkManagerError::CreateTapDevice(err.to_string()))?;
        self.taps.insert(realm_id, tap);
        info!("Created tap device for realm: {}", &realm_id);
        Ok(())
    }
    async fn shutdown_tap_device_for_realm(
        &mut self,
        realm_id: Uuid,
    ) -> Result<(), NetworkManagerError> {
        info!("Deleting tap device for realm: {}", &realm_id);
        let tap = self
            .taps
            .remove(&realm_id)
            .ok_or(NetworkManagerError::DestroyTapDevice(format!(
                "No tap device for realm: {}",
                realm_id
            )))?;
        self.bridge
            .remove_tap_device_from_bridge(tap.as_ref())
            .await
            .map_err(|err| NetworkManagerError::DestroyTapDevice(err.to_string()))?;
        info!("Deleted tap device for realm: {}", &realm_id);
        TapDeviceFabric::delete_tap(tap)
            .await
            .map_err(|err| NetworkManagerError::DestroyTapDevice(err.to_string()))
    }
}
