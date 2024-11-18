use std::{collections::HashMap, net::IpAddr};

use crate::error::ProtocolError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct NetAddr {
    pub address: IpAddr,
    pub netmask: Option<IpAddr>,
    pub destination: Option<IpAddr>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    AttestationToken(Vec<u8>),
    ApplicationExited(i32),
    ApplicationIsRunning(),
    ApplicationNotStarted(),
    IfAddrs(HashMap<String, NetAddr>),
    Success(),
    Error(ProtocolError),
}
