use std::{collections::HashMap, net::IpAddr};

use serde::{Serialize, Deserialize};
use crate::error::ProtocolError;

#[derive(Debug, Serialize, Deserialize)]
pub struct NetAddr {
    pub address: IpAddr,
    pub netmask: Option<IpAddr>,
    pub destination: Option<IpAddr>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    ApplicationExited(i32),
    ApplicationIsRunning(),
    ApplicationNotStarted(),
    IfAddrs(HashMap<String, NetAddr>),
    Success(),
    Error(ProtocolError)
}

