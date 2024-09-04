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
    #[error("Spawning udhcp Server failed: {0}")]
    SpawnFail(#[source] io::Error),
    #[error("Failed to kill udhcp Server: {0}")]
    KillFail(#[source] io::Error),
    #[error("Failed to wait for udhcp Server exit: {0}")]
    WaitFail(#[source] io::Error),
    #[error("Can't calculate address range for DHCP: {0}")]
    AddressRange(#[source] PrefixLenError),
}

pub struct DnsmasqServerHandler {
    command: Command,
    dhcp_connections_number: u8,
    child: Option<Child>,
}

impl DnsmasqServerHandler {
    pub fn new(
        path_to_exec: &Path,
        dhcp_connections_number: u8,
        dns_records: Vec<String>,
    ) -> Result<Self, DnsmasqServerError> {
        Self::validate_exec_path(path_to_exec)?;
        Ok(DnsmasqServerHandler {
            command: Self::create_command(path_to_exec, dns_records),
            dhcp_connections_number,
            child: None,
        })
    }

    fn validate_exec_path(path_to_exec: &Path) -> Result<(), DnsmasqServerError> {
        if !path_to_exec.exists() || !path_to_exec.ends_with("dnsmasq") {
            return Err(DnsmasqServerError::InvalidPath);
        }
        Ok(())
    }

    fn create_command(path_to_exec: &Path, dns_records: Vec<String>) -> Command {
        let mut command = Command::new(path_to_exec);
        command.args(vec!["-I", "lo"]); // Disable listening on LO interface
        command.args(vec!["-C", "/dev/null"]); // Disable reading config from file
        command.arg("-k"); // Keep in foreground
        command.arg("--dhcp-no-override");
        command.arg("--dhcp-authoritative");
        command.arg("--bind-dynamic");

        dns_records.into_iter().for_each(|record| {
            command.arg(format!("--address={}", record));
        });

        command.stdin(Stdio::null());
        command
    }

    fn spawn_dhcp_server(&mut self) -> Result<Child, DnsmasqServerError> {
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

    fn calculate_range_str(
        hostnet_ip: &IpNet,
        dhcp_addr_numbers: u8,
    ) -> Result<(IpAddr, IpAddr), DnsmasqServerError> {
        let start = IpNet::new(
            match hostnet_ip.addr() {
                std::net::IpAddr::V4(ip) => IpAddr::V4(ip.saturating_add(1)),
                std::net::IpAddr::V6(ip) => IpAddr::V6(ip.saturating_add(1)),
            },
            hostnet_ip.prefix_len(),
        )
        .map(|addr| addr.addr())
        .map_err(DnsmasqServerError::AddressRange)?;
        let end = IpNet::new(
            match hostnet_ip.addr() {
                std::net::IpAddr::V4(ip) => IpAddr::V4(ip.saturating_add(dhcp_addr_numbers.into())),
                std::net::IpAddr::V6(ip) => IpAddr::V6(ip.saturating_add(dhcp_addr_numbers.into())),
            },
            hostnet_ip.prefix_len(),
        )
        .map(|addr| addr.addr())
        .map_err(DnsmasqServerError::AddressRange)?;
        Ok((start, end))
    }

    async fn kill_dhcp_server(&mut self) -> Result<(), DnsmasqServerError> {
        if let Some(child) = &mut self.child {
            child.kill().await.map_err(DnsmasqServerError::KillFail)?;
            child.wait().await.map_err(DnsmasqServerError::WaitFail)?;
        }
        Ok(())
    }
}

#[async_trait]
impl DHCPServer for DnsmasqServerHandler {
    async fn start(&mut self, interface_ip: IpNet, inteface_name: &str) -> Result<(), DHCPError> {
        self.command.arg("-i").arg(inteface_name);
        let (start, end) = Self::calculate_range_str(&interface_ip, self.dhcp_connections_number)
            .map_err(|err| DHCPError::DHCPServerStart(err.to_string()))?;
        self.command.arg(format!("--dhcp-range={},{}", start, end));
        self.child = Some(
            self.spawn_dhcp_server()
                .map_err(|err| DHCPError::DHCPServerStart(err.to_string()))?,
        );
        Ok(())
    }
    async fn stop(&mut self) -> Result<(), DHCPError> {
        Ok(self
            .kill_dhcp_server()
            .await
            .map_err(|err| DHCPError::DHCPServerStop(err.to_string()))?)
    }
}
