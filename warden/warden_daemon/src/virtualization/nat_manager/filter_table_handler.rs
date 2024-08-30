use ipnet::IpNet;

use super::ip_table_handler::{IpTableHandler, IpTableHandlerError};

const TABLE_NAME: &str = "filter";
const FWI_CHAIN_NAME: &str = "DAEMONVIRT_FWI";
const FWO_CHAIN_NAME: &str = "DAEMONVIRT_FWO";
const FWX_CHAIN_NAME: &str = "DAEMONVIRT_FWX";
const INP_CHAIN_NAME: &str = "DAEMONVIRT_INP";
const OUT_CHAIN_NAME: &str = "DAEMONVIRT_OUT";
pub struct FilterIpTableManager {
    handler: iptables_wrapper::FilterIptablesTableManager,
}

impl FilterIpTableManager {
    pub fn new(if_name: String, if_ip: IpNet) -> Result<FilterIpTableManager, IpTableHandlerError> {
        Ok(Self {
            handler: iptables_wrapper::FilterIptablesTableManager::new(if_name, if_ip)
                .map_err(|err| IpTableHandlerError::HandlerError(err.to_string()))?,
        })
    }
}

impl IpTableHandler for FilterIpTableManager {
    fn insert_ip_table_rules(&self) -> Result<(), IpTableHandlerError> {
        self.handler
            .create_chain()
            .map_err(|(chain, err)| IpTableHandlerError::ChainAdd {
                chain_name: chain,
                table_name: TABLE_NAME.to_string(),
                message: err.to_string(),
            })?;
        self.handler
            .insert_ip_table_rules()
            .map_err(|(chain, err)| IpTableHandlerError::RuleAdd {
                chain_name: chain,
                table_name: TABLE_NAME.to_string(),
                message: err.to_string(),
            })
    }
    fn remove_ip_table_rules(&self) -> Result<(), IpTableHandlerError> {
        self.handler
            .delete_chain()
            .map_err(|(chain, err)| IpTableHandlerError::ChainRemove {
                chain_name: chain,
                table_name: TABLE_NAME.to_string(),
                message: err.to_string(),
            })
    }
}

mod iptables_wrapper {
    use super::{
        FWI_CHAIN_NAME, FWO_CHAIN_NAME, FWX_CHAIN_NAME, INP_CHAIN_NAME, OUT_CHAIN_NAME, TABLE_NAME,
    };
    use ipnet::IpNet;
    use iptables::IPTables;
    pub struct FilterIptablesTableManager {
        if_name: String,
        if_ip: IpNet,
        handler: IPTables,
    }

    impl FilterIptablesTableManager {
        pub fn new(if_name: String, if_ip: IpNet) -> Result<Self, Box<dyn std::error::Error>> {
            Ok(Self {
                if_name,
                if_ip,
                handler: iptables::new(if_ip.addr().is_ipv6())?,
            })
        }

        pub fn create_chain(&self) -> Result<(), (String, Box<dyn std::error::Error>)> {
            self.handler
                .new_chain("filter", FWI_CHAIN_NAME)
                .map_err(|err| (FWI_CHAIN_NAME.to_string(), err))?;
            self.handler
                .new_chain("filter", FWO_CHAIN_NAME)
                .map_err(|err| (FWO_CHAIN_NAME.to_string(), err))?;
            self.handler
                .new_chain("filter", FWX_CHAIN_NAME)
                .map_err(|err| (FWX_CHAIN_NAME.to_string(), err))?;
            self.handler
                .new_chain("filter", INP_CHAIN_NAME)
                .map_err(|err| (INP_CHAIN_NAME.to_string(), err))?;
            self.handler
                .new_chain("filter", OUT_CHAIN_NAME)
                .map_err(|err| (OUT_CHAIN_NAME.to_string(), err))?;
            let (table, chain, rule) = Self::get_input_chain_command();
            self.handler
                .append_replace(table, chain, &rule)
                .map_err(|err| (chain.to_string(), err))?;
            let (table, chain, rule) = Self::get_output_chain_command();
            self.handler
                .append_replace(table, chain, &rule)
                .map_err(|err| (chain.to_string(), err))?;
            let (table, chain, rule) = Self::get_forward_chain_command(FWI_CHAIN_NAME);
            self.handler
                .append_replace(table, chain, &rule)
                .map_err(|err| (chain.to_string(), err))?;
            let (table, chain, rule) = Self::get_forward_chain_command(FWO_CHAIN_NAME);
            self.handler
                .append_replace(table, chain, &rule)
                .map_err(|err| (chain.to_string(), err))?;
            let (table, chain, rule) = Self::get_forward_chain_command(FWX_CHAIN_NAME);
            self.handler
                .append_replace(table, chain, &rule)
                .map_err(|err| (chain.to_string(), err))
        }

        pub fn insert_ip_table_rules(&self) -> Result<(), (String, Box<dyn std::error::Error>)> {
            self.handler
                .append_replace(
                    "filter",
                    FWI_CHAIN_NAME,
                    &format!(
                        "-d {} -o {} -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT",
                        self.if_ip, &self.if_name
                    ),
                )
                .map_err(|err| (FWI_CHAIN_NAME.to_string(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    FWI_CHAIN_NAME,
                    &format!(
                        "-o {} -j REJECT --reject-with icmp-port-unreachable",
                        &self.if_name
                    ),
                )
                .map_err(|err| (FWI_CHAIN_NAME.to_string(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    FWO_CHAIN_NAME,
                    &format!("-s {} -i {} -j ACCEPT", self.if_ip, &self.if_name),
                )
                .map_err(|err| (FWO_CHAIN_NAME.to_string(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    FWO_CHAIN_NAME,
                    &format!(
                        "-i {} -j REJECT --reject-with icmp-port-unreachable",
                        &self.if_name
                    ),
                )
                .map_err(|err| (FWO_CHAIN_NAME.to_string(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    FWX_CHAIN_NAME,
                    &format!("-i {} -o {} -j ACCEPT", &self.if_name, &self.if_name),
                )
                .map_err(|err| (FWX_CHAIN_NAME.to_string(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    INP_CHAIN_NAME,
                    &format!("-i {} -p udp -m udp --dport 53 -j ACCEPT", &self.if_name),
                )
                .map_err(|err| (INP_CHAIN_NAME.to_string(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    INP_CHAIN_NAME,
                    &format!("-i {} -p tcp -m tcp --dport 53 -j ACCEPT", &self.if_name),
                )
                .map_err(|err| (INP_CHAIN_NAME.to_string(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    INP_CHAIN_NAME,
                    &format!("-i {} -p udp -m udp --dport 67 -j ACCEPT", &self.if_name),
                )
                .map_err(|err| (INP_CHAIN_NAME.to_string(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    INP_CHAIN_NAME,
                    &format!("-i {} -p tcp -m tcp --dport 67 -j ACCEPT", &self.if_name),
                )
                .map_err(|err| (INP_CHAIN_NAME.to_string(), err))?;

            self.handler
                .append_replace(
                    "filter",
                    OUT_CHAIN_NAME,
                    &format!("-o {} -p udp -m udp --dport 53 -j ACCEPT", &self.if_name),
                )
                .map_err(|err| (OUT_CHAIN_NAME.to_string(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    OUT_CHAIN_NAME,
                    &format!("-o {} -p tcp -m tcp --dport 53 -j ACCEPT", &self.if_name),
                )
                .map_err(|err| (OUT_CHAIN_NAME.to_string(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    OUT_CHAIN_NAME,
                    &format!("-o {} -p udp -m udp --dport 68 -j ACCEPT", &self.if_name),
                )
                .map_err(|err| (OUT_CHAIN_NAME.to_string(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    OUT_CHAIN_NAME,
                    &format!("-o {} -p tcp -m tcp --dport 68 -j ACCEPT", &self.if_name),
                )
                .map_err(|err| (OUT_CHAIN_NAME.to_string(), err))
        }

        pub fn delete_chain(&self) -> Result<(), (String, Box<dyn std::error::Error>)> {
            self.handler
                .flush_chain("filter", FWI_CHAIN_NAME)
                .map_err(|err| (FWI_CHAIN_NAME.to_string(), err))?;
            self.handler
                .flush_chain("filter", FWO_CHAIN_NAME)
                .map_err(|err| (FWO_CHAIN_NAME.to_string(), err))?;
            self.handler
                .flush_chain("filter", FWX_CHAIN_NAME)
                .map_err(|err| (FWX_CHAIN_NAME.to_string(), err))?;
            self.handler
                .flush_chain("filter", INP_CHAIN_NAME)
                .map_err(|err| (INP_CHAIN_NAME.to_string(), err))?;
            self.handler
                .flush_chain("filter", OUT_CHAIN_NAME)
                .map_err(|err| (OUT_CHAIN_NAME.to_string(), err))?;
            let (table, chain, rule) = Self::get_input_chain_command();
            self.handler
                .delete(table, chain, &rule)
                .map_err(|err| (chain.to_string(), err))?;
            let (table, chain, rule) = Self::get_output_chain_command();
            self.handler
                .delete(table, chain, &rule)
                .map_err(|err| (chain.to_string(), err))?;
            let (table, chain, rule) = Self::get_forward_chain_command(FWI_CHAIN_NAME);
            self.handler
                .delete(table, chain, &rule)
                .map_err(|err| (chain.to_string(), err))?;
            let (table, chain, rule) = Self::get_forward_chain_command(FWO_CHAIN_NAME);
            self.handler
                .delete(table, chain, &rule)
                .map_err(|err| (chain.to_string(), err))?;
            let (table, chain, rule) = Self::get_forward_chain_command(FWX_CHAIN_NAME);
            self.handler
                .delete(table, chain, &rule)
                .map_err(|err| (chain.to_string(), err))?;
            self.handler
                .delete_chain("filter", FWI_CHAIN_NAME)
                .map_err(|err| (FWI_CHAIN_NAME.to_string(), err))?;
            self.handler
                .delete_chain("filter", FWO_CHAIN_NAME)
                .map_err(|err| (FWO_CHAIN_NAME.to_string(), err))?;
            self.handler
                .delete_chain("filter", FWX_CHAIN_NAME)
                .map_err(|err| (FWX_CHAIN_NAME.to_string(), err))?;
            self.handler
                .delete_chain("filter", INP_CHAIN_NAME)
                .map_err(|err| (INP_CHAIN_NAME.to_string(), err))?;
            self.handler
                .delete_chain("filter", OUT_CHAIN_NAME)
                .map_err(|err| (OUT_CHAIN_NAME.to_string(), err))
        }

        fn get_input_chain_command() -> (&'static str, &'static str, String) {
            (TABLE_NAME, "INPUT", format!("-j {}", INP_CHAIN_NAME))
        }
        fn get_forward_chain_command(chain_name: &str) -> (&'static str, &'static str, String) {
            (TABLE_NAME, "FORWARD", format!("-j {}", chain_name))
        }
        fn get_output_chain_command() -> (&'static str, &'static str, String) {
            (TABLE_NAME, "OUTPUT", format!("-j {}", OUT_CHAIN_NAME))
        }
    }
}
