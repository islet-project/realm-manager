use async_trait::async_trait;
use ipnet::IpNet;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DHCPError {
    #[error("Error while launching DHCP Server: {0}")]
    DHCPServerStart(String),
    #[error("Error while stopping DHCP Server: {0}")]
    DHCPServerStop(String),
}

#[async_trait]
pub trait DHCPServer {
    async fn start(&mut self, interface_ip: IpNet, inteface_name: &str) -> Result<(), DHCPError>;
    async fn stop(&mut self) -> Result<(), DHCPError>;
}
