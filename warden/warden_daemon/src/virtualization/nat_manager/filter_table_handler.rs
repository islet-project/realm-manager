use ipnet::IpNet;

use super::ip_table_handler::{IpTableHandler, IpTableHandlerError};

const TABLE_NAME: &str = "filter";
const FWI_CHAIN_POSTFIX_NAME: &str = "FWI";
const FWO_CHAIN_POSTFIX_NAME: &str = "FWO";
const FWX_CHAIN_POSTFIX_NAME: &str = "FWX";
const INP_CHAIN_POSTFIX_NAME: &str = "INP";
const OUT_CHAIN_POSTFIX_NAME: &str = "OUT";
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
            .create_chains()
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
            .delete_chains()
            .map_err(|(chain, err)| IpTableHandlerError::ChainRemove {
                chain_name: chain,
                table_name: TABLE_NAME.to_string(),
                message: err.to_string(),
            })
    }
}

mod iptables_wrapper {
    use super::{
        FWI_CHAIN_POSTFIX_NAME, FWO_CHAIN_POSTFIX_NAME, FWX_CHAIN_POSTFIX_NAME,
        INP_CHAIN_POSTFIX_NAME, OUT_CHAIN_POSTFIX_NAME, TABLE_NAME,
    };
    use ipnet::IpNet;
    use iptables::IPTables;
    pub struct FilterIptablesTableManager {
        if_name: String,
        if_ip: IpNet,
        handler: IPTables,
        fwi_chain_name: String,
        fwo_chain_name: String,
        fwx_chain_name: String,
        inp_chain_name: String,
        out_chain_name: String,
    }

    impl FilterIptablesTableManager {
        pub fn new(if_name: String, if_ip: IpNet) -> Result<Self, Box<dyn std::error::Error>> {
            Ok(Self {
                if_name: if_name.clone(),
                if_ip,
                handler: iptables::new(if_ip.addr().is_ipv6())?,
                fwi_chain_name: format!("{}_{}", if_name, FWI_CHAIN_POSTFIX_NAME),
                fwo_chain_name: format!("{}_{}", if_name, FWO_CHAIN_POSTFIX_NAME),
                fwx_chain_name: format!("{}_{}", if_name, FWX_CHAIN_POSTFIX_NAME),
                inp_chain_name: format!("{}_{}", if_name, INP_CHAIN_POSTFIX_NAME),
                out_chain_name: format!("{}_{}", if_name, OUT_CHAIN_POSTFIX_NAME),
            })
        }

        pub fn create_chains(&self) -> Result<(), (String, Box<dyn std::error::Error>)> {
            self.handler
                .new_chain("filter", &self.fwi_chain_name)
                .map_err(|err| (self.fwi_chain_name.clone(), err))?;
            self.handler
                .new_chain("filter", &self.fwo_chain_name)
                .map_err(|err| (self.fwo_chain_name.clone(), err))?;
            self.handler
                .new_chain("filter", &self.fwx_chain_name)
                .map_err(|err| (self.fwx_chain_name.clone(), err))?;
            self.handler
                .new_chain("filter", &self.inp_chain_name)
                .map_err(|err| (self.inp_chain_name.clone(), err))?;
            self.handler
                .new_chain("filter", &self.out_chain_name)
                .map_err(|err| (self.out_chain_name.clone(), err))?;
            let (table, chain, rule) = self.get_input_chain_command();
            self.handler
                .append_replace(table, chain, &rule)
                .map_err(|err| (chain.to_string(), err))?;
            let (table, chain, rule) = self.get_output_chain_command();
            self.handler
                .append_replace(table, chain, &rule)
                .map_err(|err| (chain.to_string(), err))?;
            let (table, chain, rule) = self.get_forward_chain_command(&self.fwi_chain_name);
            self.handler
                .append_replace(table, chain, &rule)
                .map_err(|err| (chain.to_string(), err))?;
            let (table, chain, rule) = self.get_forward_chain_command(&self.fwo_chain_name);
            self.handler
                .append_replace(table, chain, &rule)
                .map_err(|err| (chain.to_string(), err))?;
            let (table, chain, rule) = self.get_forward_chain_command(&self.fwx_chain_name);
            self.handler
                .append_replace(table, chain, &rule)
                .map_err(|err| (chain.to_string(), err))
        }

        pub fn insert_ip_table_rules(&self) -> Result<(), (String, Box<dyn std::error::Error>)> {
            self.handler
                .append_replace(
                    "filter",
                    &self.fwi_chain_name,
                    &format!(
                        "-d {} -o {} -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT",
                        self.if_ip, &self.if_name
                    ),
                )
                .map_err(|err| (self.fwi_chain_name.clone(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    &self.fwi_chain_name,
                    &format!(
                        "-o {} -j REJECT --reject-with icmp-port-unreachable",
                        &self.if_name
                    ),
                )
                .map_err(|err| (self.fwi_chain_name.clone(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    &self.fwo_chain_name,
                    &format!("-s {} -i {} -j ACCEPT", self.if_ip, &self.if_name),
                )
                .map_err(|err| (self.fwo_chain_name.clone(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    &self.fwo_chain_name,
                    &format!(
                        "-i {} -j REJECT --reject-with icmp-port-unreachable",
                        &self.if_name
                    ),
                )
                .map_err(|err| (self.fwo_chain_name.clone(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    &self.fwx_chain_name,
                    &format!("-i {} -o {} -j ACCEPT", &self.if_name, &self.if_name),
                )
                .map_err(|err| (self.fwx_chain_name.clone(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    &self.inp_chain_name,
                    &format!("-i {} -p udp -m udp --dport 53 -j ACCEPT", &self.if_name),
                )
                .map_err(|err| (self.inp_chain_name.clone(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    &self.inp_chain_name,
                    &format!("-i {} -p tcp -m tcp --dport 53 -j ACCEPT", &self.if_name),
                )
                .map_err(|err| (self.inp_chain_name.clone(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    &self.inp_chain_name,
                    &format!("-i {} -p udp -m udp --dport 67 -j ACCEPT", &self.if_name),
                )
                .map_err(|err| (self.inp_chain_name.clone(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    &self.inp_chain_name,
                    &format!("-i {} -p tcp -m tcp --dport 67 -j ACCEPT", &self.if_name),
                )
                .map_err(|err| (self.inp_chain_name.clone(), err))?;

            self.handler
                .append_replace(
                    "filter",
                    &self.out_chain_name,
                    &format!("-o {} -p udp -m udp --dport 53 -j ACCEPT", &self.if_name),
                )
                .map_err(|err| (self.out_chain_name.clone(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    &self.out_chain_name,
                    &format!("-o {} -p tcp -m tcp --dport 53 -j ACCEPT", &self.if_name),
                )
                .map_err(|err| (self.out_chain_name.clone(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    &self.out_chain_name,
                    &format!("-o {} -p udp -m udp --dport 68 -j ACCEPT", &self.if_name),
                )
                .map_err(|err| (self.out_chain_name.clone(), err))?;
            self.handler
                .append_replace(
                    "filter",
                    &self.out_chain_name,
                    &format!("-o {} -p tcp -m tcp --dport 68 -j ACCEPT", &self.if_name),
                )
                .map_err(|err| (self.out_chain_name.clone(), err))
        }

        pub fn delete_chains(&self) -> Result<(), (String, Box<dyn std::error::Error>)> {
            self.handler
                .flush_chain("filter", &self.fwi_chain_name)
                .map_err(|err| (self.fwi_chain_name.clone(), err))?;
            self.handler
                .flush_chain("filter", &self.fwo_chain_name)
                .map_err(|err| (self.fwo_chain_name.clone(), err))?;
            self.handler
                .flush_chain("filter", &self.fwx_chain_name)
                .map_err(|err| (self.fwx_chain_name.clone(), err))?;
            self.handler
                .flush_chain("filter", &self.inp_chain_name)
                .map_err(|err| (self.inp_chain_name.clone(), err))?;
            self.handler
                .flush_chain("filter", &self.out_chain_name)
                .map_err(|err| (self.out_chain_name.clone(), err))?;
            let (table, chain, rule) = self.get_input_chain_command();
            self.handler
                .delete(table, chain, &rule)
                .map_err(|err| (chain.to_string(), err))?;
            let (table, chain, rule) = self.get_output_chain_command();
            self.handler
                .delete(table, chain, &rule)
                .map_err(|err| (chain.to_string(), err))?;
            let (table, chain, rule) = self.get_forward_chain_command(&self.fwi_chain_name);
            self.handler
                .delete(table, chain, &rule)
                .map_err(|err| (chain.to_string(), err))?;
            let (table, chain, rule) = self.get_forward_chain_command(&self.fwo_chain_name);
            self.handler
                .delete(table, chain, &rule)
                .map_err(|err| (chain.to_string(), err))?;
            let (table, chain, rule) = self.get_forward_chain_command(&self.fwx_chain_name);
            self.handler
                .delete(table, chain, &rule)
                .map_err(|err| (chain.to_string(), err))?;
            self.handler
                .delete_chain("filter", &self.fwi_chain_name)
                .map_err(|err| (self.fwi_chain_name.clone(), err))?;
            self.handler
                .delete_chain("filter", &self.fwo_chain_name)
                .map_err(|err| (self.fwo_chain_name.clone(), err))?;
            self.handler
                .delete_chain("filter", &self.fwx_chain_name)
                .map_err(|err| (self.fwx_chain_name.clone(), err))?;
            self.handler
                .delete_chain("filter", &self.inp_chain_name)
                .map_err(|err| (self.inp_chain_name.clone(), err))?;
            self.handler
                .delete_chain("filter", &self.out_chain_name)
                .map_err(|err| (self.out_chain_name.clone(), err))
        }

        fn get_input_chain_command(&self) -> (&'static str, &'static str, String) {
            (TABLE_NAME, "INPUT", format!("-j {}", &self.inp_chain_name))
        }
        fn get_forward_chain_command(
            &self,
            chain_name: &str,
        ) -> (&'static str, &'static str, String) {
            (TABLE_NAME, "FORWARD", format!("-j {}", chain_name))
        }
        fn get_output_chain_command(&self) -> (&'static str, &'static str, String) {
            (TABLE_NAME, "OUTPUT", format!("-j {}", &self.out_chain_name))
        }
    }
}
