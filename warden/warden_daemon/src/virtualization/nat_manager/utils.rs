use std::net::IpAddr;

pub fn create_network_string(ip: IpAddr, mask: u8) -> String {
    let network_ip = match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            format!("{}.{}.{}.0", octets[0], octets[1], octets[2])
        }
        IpAddr::V6(_v6) => String::new(),
    };
    format!("{}/{}", network_ip, mask)
}
