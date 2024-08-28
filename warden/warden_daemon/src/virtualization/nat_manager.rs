use std::{collections::HashMap, net::IpAddr};

use async_trait::async_trait;
use futures::TryStreamExt;
use rtnetlink::{new_connection, Handle};
use tokio_tun::TunBuilder;
use uuid::Uuid;

use super::network_manager::{NetworkManager, NetworkManagerError};

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
    const NAT_CHAIN_NAME: &'static str = "DAEMONVIRT_PRT";
    const FILTER_FWI_CHAIN_NAME: &'static str = "DAEMONVIRT_FWI";
    const FILTER_FWO_CHAIN_NAME: &'static str = "DAEMONVIRT_FWO";
    const FILTER_FWX_CHAIN_NAME: &'static str = "DAEMONVIRT_FWX";
    const FILTER_INP_CHAIN_NAME: &'static str = "DAEMONVIRT_INP";
    const FILTER_OUT_CHAIN_NAME: &'static str = "DAEMONVIRT_OUT";

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

    fn create_network_string(&self) -> String {
        let network_ip = match self.config.bridge_ip {
            IpAddr::V4(v4) => {
                let octets = v4.octets();
                format!("{}.{}.{}.0", octets[0], octets[1], octets[2])
            }
            IpAddr::V6(v6) => String::new(),
        };
        format!("{}/{}", network_ip, self.config.bridge_mask)
    }

    async fn cleanup_routing(&self) -> Result<(), NetworkManagerError> {
        let ip_tables_handle = iptables::new(false)
            .map_err(|err| NetworkManagerError::IpTablesOperation(err.to_string()))?;
        let network_string = self.create_network_string();

        let _ = ip_tables_handle.delete_all("mangle", Self::NAT_CHAIN_NAME, &format!("-o {} -p udp -m udp --dport 68 -j CHECKSUM --checksum-fill", &self.config.bridge_name));
        let _ = ip_tables_handle.delete_all("mangle", "POSTROUTING", &format!("-j {}", Self::NAT_CHAIN_NAME));
        let _ = ip_tables_handle.delete_chain("mangle", Self::NAT_CHAIN_NAME);

        let _ = ip_tables_handle.delete_all(
            "nat",
            "POSTROUTING",
            &format!("-j {}", Self::NAT_CHAIN_NAME),
        );
        let _ = ip_tables_handle.delete_all(
            "nat",
            "POSTROUTING",
            &format!("-o eno1 -j {}", Self::NAT_CHAIN_NAME),
        ); // DELETE
        let _ = ip_tables_handle.delete_all(
            "nat",
            Self::NAT_CHAIN_NAME,
            &format!("-s {} -d 224.0.0.0/24 -j RETURN", &network_string),
        ); // DELETE
        let _ = ip_tables_handle.delete_all(
            "nat",
            Self::NAT_CHAIN_NAME,
            &format!("-s {} -d 225.255.255.255 -j RETURN", &network_string),
        ); // DELETE
        let _ = ip_tables_handle.delete_all(
            "nat",
            Self::NAT_CHAIN_NAME,
            &format!(
                "-s {} ! -d {} -p tcp -j MASQUERADE --to-ports 1024-65535",
                &network_string, &network_string
            ),
        );
        let _ = ip_tables_handle.delete_all(
            "nat",
            Self::NAT_CHAIN_NAME,
            &format!(
                "-s {} ! -d {} -p udp -j MASQUERADE --to-ports 1024-65535",
                &network_string, &network_string
            ),
        );
        let _ = ip_tables_handle.delete_all(
            "nat",
            Self::NAT_CHAIN_NAME,
            &format!(
                "-s {} ! -d {} -j MASQUERADE",
                &network_string, &network_string
            ),
        );
        let _ = ip_tables_handle.delete_chain("nat", Self::NAT_CHAIN_NAME);

        let _ = ip_tables_handle.delete(
            "filter",
            "INPUT",
            &format!("-j {}", Self::FILTER_INP_CHAIN_NAME),
        );
        let _ = ip_tables_handle.delete(
            "filter",
            "FORWARD",
            &format!("-j {}", Self::FILTER_FWX_CHAIN_NAME),
        );
        let _ = ip_tables_handle.delete(
            "filter",
            "FORWARD",
            &format!("-j {}", Self::FILTER_FWI_CHAIN_NAME),
        );
        let _ = ip_tables_handle.delete(
            "filter",
            "FORWARD",
            &format!("-j {}", Self::FILTER_FWO_CHAIN_NAME),
        );
        let _ = ip_tables_handle.delete(
            "filter",
            "OUTPUT",
            &format!("-j {}", Self::FILTER_OUT_CHAIN_NAME),
        );
        let _ = ip_tables_handle.delete(
            "filter",
            Self::FILTER_FWI_CHAIN_NAME,
            &format!(
                "-d {} -o {} -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT",
                &network_string, &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.delete(
            "filter",
            Self::FILTER_FWI_CHAIN_NAME,
            &format!(
                "-o {} -j REJECT --reject-with icmp-port-unreachable",
                &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.delete(
            "filter",
            Self::FILTER_FWO_CHAIN_NAME,
            &format!(
                "-s {} -i {} -j ACCEPT",
                &network_string, &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.delete(
            "filter",
            Self::FILTER_FWO_CHAIN_NAME,
            &format!(
                "-i {} -j REJECT --reject-with icmp-port-unreachable",
                &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.delete(
            "filter",
            Self::FILTER_FWX_CHAIN_NAME,
            &format!(
                "-i {} -o {} -j ACCEPT",
                &self.config.bridge_name, &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.delete(
            "filter",
            Self::FILTER_INP_CHAIN_NAME,
            &format!(
                "-i {} -p udp -m udp --dport 53 -j ACCEPT",
                &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.delete(
            "filter",
            Self::FILTER_INP_CHAIN_NAME,
            &format!(
                "-i {} -p tcp -m tcp --dport 53 -j ACCEPT",
                &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.delete(
            "filter",
            Self::FILTER_INP_CHAIN_NAME,
            &format!(
                "-i {} -p udp -m udp --dport 67 -j ACCEPT",
                &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.delete(
            "filter",
            Self::FILTER_INP_CHAIN_NAME,
            &format!(
                "-i {} -p tcp -m tcp --dport 67 -j ACCEPT",
                &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.delete(
            "filter",
            Self::FILTER_OUT_CHAIN_NAME,
            &format!(
                "-o {} -p udp -m udp --dport 53 -j ACCEPT",
                &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.delete(
            "filter",
            Self::FILTER_OUT_CHAIN_NAME,
            &format!(
                "-o {} -p tcp -m tcp --dport 53 -j ACCEPT",
                &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.delete(
            "filter",
            Self::FILTER_OUT_CHAIN_NAME,
            &format!(
                "-o {} -p udp -m udp --dport 68 -j ACCEPT",
                &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.delete(
            "filter",
            Self::FILTER_OUT_CHAIN_NAME,
            &format!(
                "-o {} -p tcp -m tcp --dport 68 -j ACCEPT",
                &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.delete_chain("filter", Self::FILTER_FWI_CHAIN_NAME);
        let _ = ip_tables_handle.delete_chain("filter", Self::FILTER_FWO_CHAIN_NAME);
        let _ = ip_tables_handle.delete_chain("filter", Self::FILTER_FWX_CHAIN_NAME);
        let _ = ip_tables_handle.delete_chain("filter", Self::FILTER_INP_CHAIN_NAME);
        let _ = ip_tables_handle.delete_chain("filter", Self::FILTER_OUT_CHAIN_NAME);
        Ok(())
    }

    async fn prepare_routing(&self) -> Result<(), NetworkManagerError> {
        let network_string = self.create_network_string();
        let ip_tables_handle = iptables::new(false)
            .map_err(|err| NetworkManagerError::IpTablesOperation(err.to_string()))?;
        
        let _ = ip_tables_handle.new_chain("mangle", Self::NAT_CHAIN_NAME);
        let _ = ip_tables_handle.append(
            "mangle",
            "POSTROUTING",
            &format!("-j {}", Self::NAT_CHAIN_NAME),
        );
        let _ = ip_tables_handle.append("mangle", Self::NAT_CHAIN_NAME, &format!("-o {} -p udp -m udp --dport 68 -j CHECKSUM --checksum-fill", &self.config.bridge_name));
        
        let _ = ip_tables_handle.new_chain("nat", Self::NAT_CHAIN_NAME);
        let _ = ip_tables_handle.append(
            "nat",
            "POSTROUTING",
            &format!("-j {}", Self::NAT_CHAIN_NAME),
        );
        let _ = ip_tables_handle.append(
            "nat",
            Self::NAT_CHAIN_NAME,
            &format!("-s {} -d 224.0.0.0/24 -j RETURN", &network_string),
        ); // DELETE
        let _ = ip_tables_handle.append(
            "nat",
            Self::NAT_CHAIN_NAME,
            &format!("-s {} -d 225.255.255.255 -j RETURN", &network_string),
        ); // DELETE
        let _ = ip_tables_handle.append(
            "nat",
            Self::NAT_CHAIN_NAME,
            &format!(
                "-s {} ! -d {} -p tcp -j MASQUERADE --to-ports 1024-65535",
                &network_string, &network_string
            ),
        );
        let _ = ip_tables_handle.append(
            "nat",
            Self::NAT_CHAIN_NAME,
            &format!(
                "-s {} ! -d {} -p udp -j MASQUERADE --to-ports 1024-65535",
                &network_string, &network_string
            ),
        );
        let _ = ip_tables_handle.append(
            "nat",
            Self::NAT_CHAIN_NAME,
            &format!(
                "-s {} ! -d {} -j MASQUERADE",
                &network_string, &network_string
            ),
        );

        let _ = ip_tables_handle.new_chain("filter", Self::FILTER_FWI_CHAIN_NAME);
        let _ = ip_tables_handle.new_chain("filter", Self::FILTER_FWO_CHAIN_NAME);
        let _ = ip_tables_handle.new_chain("filter", Self::FILTER_FWX_CHAIN_NAME);
        let _ = ip_tables_handle.new_chain("filter", Self::FILTER_INP_CHAIN_NAME);
        let _ = ip_tables_handle.new_chain("filter", Self::FILTER_OUT_CHAIN_NAME);

        let _ = ip_tables_handle.append(
            "filter",
            "INPUT",
            &format!("-j {}", Self::FILTER_INP_CHAIN_NAME),
        );
        let _ = ip_tables_handle.append(
            "filter",
            "FORWARD",
            &format!("-j {}", Self::FILTER_FWX_CHAIN_NAME),
        );
        let _ = ip_tables_handle.append(
            "filter",
            "FORWARD",
            &format!("-j {}", Self::FILTER_FWI_CHAIN_NAME),
        );
        let _ = ip_tables_handle.append(
            "filter",
            "FORWARD",
            &format!("-j {}", Self::FILTER_FWO_CHAIN_NAME),
        );
        let _ = ip_tables_handle.append(
            "filter",
            "OUTPUT",
            &format!("-j {}", Self::FILTER_OUT_CHAIN_NAME),
        );

        let _ = ip_tables_handle.append(
            "filter",
            Self::FILTER_FWI_CHAIN_NAME,
            &format!(
                "-d {} -o {} -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT",
                &network_string, &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.append(
            "filter",
            Self::FILTER_FWI_CHAIN_NAME,
            &format!(
                "-o {} -j REJECT --reject-with icmp-port-unreachable",
                &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.append(
            "filter",
            Self::FILTER_FWO_CHAIN_NAME,
            &format!(
                "-s {} -i {} -j ACCEPT",
                &network_string, &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.append(
            "filter",
            Self::FILTER_FWO_CHAIN_NAME,
            &format!(
                "-i {} -j REJECT --reject-with icmp-port-unreachable",
                &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.append(
            "filter",
            Self::FILTER_FWX_CHAIN_NAME,
            &format!(
                "-i {} -o {} -j ACCEPT",
                &self.config.bridge_name, &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.append(
            "filter",
            Self::FILTER_INP_CHAIN_NAME,
            &format!(
                "-i {} -p udp -m udp --dport 53 -j ACCEPT",
                &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.append(
            "filter",
            Self::FILTER_INP_CHAIN_NAME,
            &format!(
                "-i {} -p tcp -m tcp --dport 53 -j ACCEPT",
                &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.append(
            "filter",
            Self::FILTER_INP_CHAIN_NAME,
            &format!(
                "-i {} -p udp -m udp --dport 67 -j ACCEPT",
                &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.append(
            "filter",
            Self::FILTER_INP_CHAIN_NAME,
            &format!(
                "-i {} -p tcp -m tcp --dport 67 -j ACCEPT",
                &self.config.bridge_name
            ),
        );

        let _ = ip_tables_handle.append(
            "filter",
            Self::FILTER_OUT_CHAIN_NAME,
            &format!(
                "-o {} -p udp -m udp --dport 53 -j ACCEPT",
                &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.append(
            "filter",
            Self::FILTER_OUT_CHAIN_NAME,
            &format!(
                "-o {} -p tcp -m tcp --dport 53 -j ACCEPT",
                &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.append(
            "filter",
            Self::FILTER_OUT_CHAIN_NAME,
            &format!(
                "-o {} -p udp -m udp --dport 68 -j ACCEPT",
                &self.config.bridge_name
            ),
        );
        let _ = ip_tables_handle.append(
            "filter",
            Self::FILTER_OUT_CHAIN_NAME,
            &format!(
                "-o {} -p tcp -m tcp --dport 68 -j ACCEPT",
                &self.config.bridge_name
            ),
        );
        Ok(())
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
