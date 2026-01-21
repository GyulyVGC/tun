#![allow(clippy::module_name_repetitions)]

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::FORWARD_PORT;
use serde::{Deserialize, Serialize};

/// Struct containing all peers information.
#[derive(Default)]
pub struct Peers {
    /// Mapping from veth addresses to Ethernet IPs.
    ips: HashMap<VethKey, Ipv4Addr>,
}

impl Peers {
    pub fn get_socket_by_veth(&self, veth_key: VethKey) -> Option<SocketAddr> {
        self.ips
            .get(&veth_key)
            .map(|ip| SocketAddr::new(IpAddr::V4(*ip), FORWARD_PORT))
    }

    pub fn insert(&mut self, veth_key: VethKey, eth_ip: Ipv4Addr) {
        self.ips.insert(veth_key, eth_ip);
    }
}

/// Struct identifying veth on a VLAN.
#[derive(Eq, Hash, PartialEq, Clone, Copy, Serialize, Deserialize, Debug)]
#[serde(rename = "veth")]
pub struct VethKey {
    /// IP address of the veth.
    veth_ip: Ipv4Addr,
    /// VLAN ID of the veth.
    vlan_id: u16,
}

impl VethKey {
    pub fn new(veth_ip: Ipv4Addr, vlan_id: u16) -> Self {
        Self { veth_ip, vlan_id }
    }
}
