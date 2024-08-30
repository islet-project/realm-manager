use ipnet::IpNet;

use super::ip_table_handler::{IpTableHandler, IpTableHandlerError};

const TABLE_NAME: &str = "mangle";
const CHAIN_NAME: &str = "DAEMONVIRT_PRT";

pub struct MangleTableManager {
    handler: iptables_wrapper::MangleIptablesTableManager,
}

impl MangleTableManager {
    pub fn new(if_name: String, if_ip: IpNet) -> Result<MangleTableManager, IpTableHandlerError> {
        Ok(Self {
            handler: iptables_wrapper::MangleIptablesTableManager::new(if_name, if_ip)
                .map_err(|err| IpTableHandlerError::HandlerError(err.to_string()))?,
        })
    }
}

impl IpTableHandler for MangleTableManager {
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

    pub struct MangleIptablesTableManager {
        handler: IPTables,
        if_name: String,
    }

    impl MangleIptablesTableManager {
        pub fn new(if_name: String, if_ip: IpNet) -> Result<Self, Box<dyn std::error::Error>> {
            Ok(Self {
                if_name,
                handler: iptables::new(if_ip.addr().is_ipv6())?,
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
                &format!(
                    "-o {} -p udp -m udp --dport 68 -j CHECKSUM --checksum-fill",
                    &self.if_name
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
