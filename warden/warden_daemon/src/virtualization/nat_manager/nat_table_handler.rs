use std::net::IpAddr;

use super::ip_table_handler::{IpTableHandler, IpTableHandlerError};

const TABLE_NAME: &'static str = "nat";
const CHAIN_NAME: &'static str = "DAEMONVIRT_PRT";

pub struct NatIpTableManager {
    handler: iptables_wrapper::NatIptablesTableManager,
}

impl NatIpTableManager {
    pub fn new(if_ip: IpAddr, if_mask: u8) -> Result<impl IpTableHandler, IpTableHandlerError> {
        Ok(Self {
            handler: iptables_wrapper::NatIptablesTableManager::new(if_ip, if_mask)
                .map_err(|err| IpTableHandlerError::HandlerError(err.to_string()))?,
        })
    }
}

impl IpTableHandler for NatIpTableManager {
    fn insert_ip_table_rules(&self) -> Result<(), IpTableHandlerError> {
        self.handler
            .create_chain()
            .map_err(|err| IpTableHandlerError::ChainAdd {
                chain_name: CHAIN_NAME.to_string(),
                table_name: TABLE_NAME.to_string(),
                message: err.to_string(),
            })?;
        self.handler
            .insert_ip_table_rules()
            .map_err(|err| IpTableHandlerError::RuleAdd {
                chain_name: CHAIN_NAME.to_string(),
                table_name: TABLE_NAME.to_string(),
                message: err.to_string(),
            })
    }
    fn remove_ip_table_rules(&self) -> Result<(), IpTableHandlerError> {
        self.handler
            .delete_chain()
            .map_err(|err| IpTableHandlerError::ChainRemove {
                chain_name: CHAIN_NAME.to_string(),
                table_name: TABLE_NAME.to_string(),
                message: err.to_string(),
            })
    }
}

mod iptables_wrapper {
    use super::{IpAddr, CHAIN_NAME, TABLE_NAME};
    use iptables::IPTables;

    use crate::virtualization::nat_manager::utils::create_network_string;

    pub struct NatIptablesTableManager {
        handler: IPTables,
        network_string: String,
    }

    impl NatIptablesTableManager {
        pub fn new(if_ip: IpAddr, if_mask: u8) -> Result<Self, Box<dyn std::error::Error>> {
            Ok(Self {
                handler: iptables::new(if_ip.is_ipv6())?,
                network_string: create_network_string(if_ip, if_mask),
            })
        }

        pub fn create_chain(&self) -> Result<(), Box<dyn std::error::Error>> {
            if !self.handler.chain_exists(TABLE_NAME, CHAIN_NAME)? {
                self.handler.new_chain(TABLE_NAME, CHAIN_NAME)?;
            }
            let (table, chain, rule) = Self::get_postrouting_chain_command();
            self.handler.append_replace(table, chain, &rule)
        }

        pub fn insert_ip_table_rules(&self) -> Result<(), Box<dyn std::error::Error>> {
            self.handler.append_replace(
                TABLE_NAME,
                CHAIN_NAME,
                &format!("-s {} -d 224.0.0.0/24 -j RETURN", &self.network_string),
            )?;
            self.handler.append_replace(
                TABLE_NAME,
                CHAIN_NAME,
                &format!("-s {} -d 225.255.255.255 -j RETURN", &self.network_string),
            )?; // DELETE
            self.handler.append_replace(
                TABLE_NAME,
                CHAIN_NAME,
                &format!(
                    "-s {} ! -d {} -p tcp -j MASQUERADE --to-ports 1024-65535",
                    &self.network_string, &self.network_string
                ),
            )?;
            self.handler.append_replace(
                TABLE_NAME,
                CHAIN_NAME,
                &format!(
                    "-s {} ! -d {} -p udp -j MASQUERADE --to-ports 1024-65535",
                    &self.network_string, &self.network_string
                ),
            )?;
            self.handler.append_replace(
                TABLE_NAME,
                CHAIN_NAME,
                &format!(
                    "-s {} ! -d {} -j MASQUERADE",
                    &self.network_string, &self.network_string
                ),
            )
        }

        pub fn delete_chain(&self) -> Result<(), Box<dyn std::error::Error>> {
            self.handler.flush_chain(TABLE_NAME, CHAIN_NAME)?;
            let (table, chain, rule) = Self::get_postrouting_chain_command();
            self.handler.delete(table, chain, &rule)?;
            self.handler.delete_chain(TABLE_NAME, CHAIN_NAME)
        }

        fn get_postrouting_chain_command() -> (&'static str, &'static str, String) {
            (TABLE_NAME, "POSTROUTING", format!("-j {}", CHAIN_NAME))
        }
    }
}
