use ipnet::IpNet;

use super::ip_table_handler::{IpTableHandler, IpTableHandlerError};

const TABLE_NAME: &str = "nat";
const CHAIN_NAME: &str = "DAEMONVIRT_PRT";

pub struct NatIpTableManager {
    handler: iptables_wrapper::NatIptablesTableManager,
}

impl NatIpTableManager {
    pub fn new(if_ip: IpNet) -> Result<NatIpTableManager, IpTableHandlerError> {
        Ok(Self {
            handler: iptables_wrapper::NatIptablesTableManager::new(if_ip)
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
    use super::{CHAIN_NAME, TABLE_NAME};
    use ipnet::IpNet;
    use iptables::IPTables;

    pub struct NatIptablesTableManager {
        handler: IPTables,
        if_ip: IpNet,
    }

    impl NatIptablesTableManager {
        pub fn new(if_ip: IpNet) -> Result<Self, Box<dyn std::error::Error>> {
            Ok(Self {
                handler: iptables::new(if_ip.addr().is_ipv6())?,
                if_ip,
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
            let network_string = self.if_ip.to_string();
            self.handler.append_replace(
                TABLE_NAME,
                CHAIN_NAME,
                &format!("-s {} -d 224.0.0.0/24 -j RETURN", network_string),
            )?;
            self.handler.append_replace(
                TABLE_NAME,
                CHAIN_NAME,
                &format!("-s {} -d 225.255.255.255 -j RETURN", network_string),
            )?;
            self.handler.append_replace(
                TABLE_NAME,
                CHAIN_NAME,
                &format!(
                    "-s {} ! -d {} -p tcp -j MASQUERADE --to-ports 1024-65535",
                    network_string, network_string
                ),
            )?;
            self.handler.append_replace(
                TABLE_NAME,
                CHAIN_NAME,
                &format!(
                    "-s {} ! -d {} -p udp -j MASQUERADE --to-ports 1024-65535",
                    network_string, network_string
                ),
            )?;
            self.handler.append_replace(
                TABLE_NAME,
                CHAIN_NAME,
                &format!(
                    "-s {} ! -d {} -j MASQUERADE",
                    network_string, network_string
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
