use std::collections::HashMap;
use std::net::IpAddr;

use nix::errno::Errno;
use nix::sys::socket::{AddressFamily, SockaddrLike, SockaddrStorage};
use thiserror::Error;
use tokio::task::block_in_place;
use warden_realm::NetAddr;

#[derive(Debug, Error)]
pub enum NetError {
    #[error("Failed to read ip addresses of network interfaces")]
    GetIfAddrsError(#[from] Errno),
}

use super::Result;

pub fn convert_sock_addr(storage: SockaddrStorage) -> Option<IpAddr> {
    match storage.family() {
        Some(AddressFamily::Inet) => storage.as_sockaddr_in().map(|addr| IpAddr::V4(addr.ip().into())),
        Some(AddressFamily::Inet6) => storage.as_sockaddr_in6().map(|addr| IpAddr::V6(addr.ip())),
        _ => None,
    }
}

pub fn read_if_addrs() -> Result<HashMap<String, NetAddr>> {
    let ifaddrs = block_in_place(nix::ifaddrs::getifaddrs).map_err(NetError::GetIfAddrsError)?;

    let mut net_addrs = HashMap::new();

    for ifaddr in ifaddrs {
        let ipaddr = ifaddr.address.and_then(convert_sock_addr);

        if let Some(addr) = ipaddr {
            net_addrs.insert(
                ifaddr.interface_name,
                NetAddr {
                    address: addr,
                    netmask: ifaddr.netmask.and_then(convert_sock_addr),
                    destination: ifaddr.destination.and_then(convert_sock_addr),
                },
            );
        }
    }

    Ok(net_addrs)
}
