use std::{io, net::IpAddr, path::Path, process::Stdio};

use async_trait::async_trait;
use ipnet::{IpAdd, IpNet, PrefixLenError};
use thiserror::Error;
use tokio::process::{Child, Command};

use super::dhcp::{DHCPError, DHCPServer};

#[derive(Debug, Error)]
pub enum DnsmasqServerError {
    #[error("Path to Dnsmasq server is invalid.")]
    InvalidPath,
    #[error("Dnsmasq Server already has been started.")]
    AlreadyStarted,
    #[error("Spawning Dnsmasq failed: {0}")]
    SpawnFail(#[source] io::Error),
    #[error("Failed to kill Dnsmasq: {0}")]
    KillFail(#[source] io::Error),
    #[error("Failed to wait for Dnsmasq exit: {0}")]
    WaitFail(#[source] io::Error),
    #[error("Can't calculate address range for DHCP: {0}")]
    AddressRange(#[source] PrefixLenError),
}

const EXEC_NAME: &str = "dnsmasq";
pub struct DnsmasqServerHandler {
    command: Command,
    dhcp_connections_number: u8,
    child: Option<Child>,
}

impl DnsmasqServerHandler {
    pub fn new(
        path_to_exec: &Path,
        dhcp_connections_number: u8,
    ) -> Result<Self, DnsmasqServerError> {
        Self::validate_exec_path(path_to_exec)?;
        Ok(DnsmasqServerHandler {
            command: Self::create_command(path_to_exec),
            dhcp_connections_number,
            child: None,
        })
    }

    pub fn add_dns_args(&mut self, dns_records: Vec<String>) {
        dns_records.into_iter().for_each(|record| {
            self.command.arg(format!("--address={}", record));
        });
    }

    fn validate_exec_path(path_to_exec: &Path) -> Result<(), DnsmasqServerError> {
        if !path_to_exec.exists() || !path_to_exec.ends_with(EXEC_NAME) {
            return Err(DnsmasqServerError::InvalidPath);
        }
        Ok(())
    }

    fn create_command(path_to_exec: &Path) -> Command {
        let mut command = Command::new(path_to_exec);
        Self::add_dhcp_args(&mut command);

        command.stdin(Stdio::null());
        command
    }

    fn add_dhcp_args(command: &mut Command) {
        command.args(vec!["-I", "lo"]); // Disable listening on LO interface
        command.args(vec!["-C", "/dev/null"]); // Disable reading config from file
        command.arg("-k"); // Keep in foreground
        command.arg("--dhcp-no-override");
        command.arg("--dhcp-authoritative");
        command.arg("--bind-dynamic");
    }

    fn calculate_next_addr(hostnet_ip: &IpNet, offset: u8) -> Result<IpNet, PrefixLenError> {
        IpNet::new(
            match hostnet_ip.addr() {
                std::net::IpAddr::V4(ip) => IpAddr::V4(ip.saturating_add(offset.into())),
                std::net::IpAddr::V6(ip) => IpAddr::V6(ip.saturating_add(offset.into())),
            },
            hostnet_ip.prefix_len(),
        )
    }

    fn calculate_range_str(
        hostnet_ip: &IpNet,
        dhcp_addr_numbers: u8,
    ) -> Result<(IpAddr, IpAddr), DnsmasqServerError> {
        const DHCP_POOL_START_OFFSET: u8 = 1;
        let start = Self::calculate_next_addr(hostnet_ip, DHCP_POOL_START_OFFSET)
            .map(|addr| addr.addr())
            .map_err(DnsmasqServerError::AddressRange)?;
        let end = Self::calculate_next_addr(hostnet_ip, dhcp_addr_numbers)
            .map(|addr| addr.addr())
            .map_err(DnsmasqServerError::AddressRange)?;
        Ok((start, end))
    }

    async fn kill_server(&mut self) -> Result<(), DnsmasqServerError> {
        if let Some(child) = &mut self.child {
            child.kill().await.map_err(DnsmasqServerError::KillFail)?;
            child.wait().await.map_err(DnsmasqServerError::WaitFail)?;
        }
        Ok(())
    }

    fn spawn_server(
        &mut self,
        interface_ip: IpNet,
        inteface_name: &str,
    ) -> Result<(), DnsmasqServerError> {
        let (start, end) = Self::calculate_range_str(&interface_ip, self.dhcp_connections_number)?;
        self.command.arg("-i").arg(inteface_name);
        self.command.arg(format!("--dhcp-range={},{}", start, end));
        self.child = Some(self.spawn_command()?);
        Ok(())
    }

    fn spawn_command(&mut self) -> Result<Child, DnsmasqServerError> {
        if self.child.is_some() {
            Err(DnsmasqServerError::AlreadyStarted)
        } else {
            let spawned_exec = self
                .command
                .spawn()
                .map_err(DnsmasqServerError::SpawnFail)?;
            Ok(spawned_exec)
        }
    }
}

#[async_trait]
impl DHCPServer for DnsmasqServerHandler {
    async fn start(&mut self, interface_ip: IpNet, inteface_name: &str) -> Result<(), DHCPError> {
        self.spawn_server(interface_ip, inteface_name)
            .map_err(|err| DHCPError::DHCPServerStart(err.to_string()))
    }
    async fn stop(&mut self) -> Result<(), DHCPError> {
        self.kill_server()
            .await
            .map_err(|err| DHCPError::DHCPServerStop(err.to_string()))
    }
}
