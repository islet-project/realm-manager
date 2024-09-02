use std::{io, path::Path, process::Stdio};

use async_trait::async_trait;
use ipnet::IpNet;
use thiserror::Error;
use tokio::process::{Child, Command};

use super::dhcp::{DHCPError, DHCPServer};

#[derive(Debug, Error)]
pub enum UDHCPDServerError {
    #[error("Path to Udhcp server is invalid.")]
    InvalidPath,
    #[error("Udhcp Server already has been started.")]
    AlreadyStarted,
    #[error("Spawning udhcp Server failed: {0}")]
    SpawnFail(#[source] io::Error),
    #[error("Failed to kill udhcp Server: {0}")]
    KillFail(#[source] io::Error),
    #[error("Failed to wait for udhcp Server exit: {0}")]
    WaitFail(#[source] io::Error),
}

pub struct UDHCPServerHandler {
    command: Command,
    child: Option<Child>,
}

impl UDHCPServerHandler {
    pub fn new(path_to_exec: &Path) -> Result<Self, UDHCPDServerError> {
        if !path_to_exec.exists() || !path_to_exec.ends_with("udhcpd") {
            return Err(UDHCPDServerError::InvalidPath);
        }
        let mut command = Command::new(path_to_exec);
        command.arg("-fS");
        command.stdin(Stdio::null());
        Ok(UDHCPServerHandler {
            command,
            child: None,
        })
    }

    fn spawn_dhcp_server(&mut self) -> Result<Child, UDHCPDServerError> {
        if self.child.is_some() {
            Err(UDHCPDServerError::AlreadyStarted)
        } else {
            let spawned_exec = self.command.spawn().map_err(UDHCPDServerError::SpawnFail)?;
            Ok(spawned_exec)
        }
    }
    async fn kill_dhcp_server(&mut self) -> Result<(), UDHCPDServerError> {
        if let Some(child) = &mut self.child {
            child.kill().await.map_err(UDHCPDServerError::KillFail)?;
            child.wait().await.map_err(UDHCPDServerError::WaitFail)?;
        }
        Ok(())
    }
}

#[async_trait]
impl DHCPServer for UDHCPServerHandler {
    async fn start(&mut self, interface_ip: IpNet) -> Result<(), DHCPError> {
        self.command.arg("-I").arg(interface_ip.addr().to_string());
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
