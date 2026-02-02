use crate::ovs::helpers::configure_access_port;
use crate::peers::peer::VethKey;
use ipnetwork::Ipv4Network;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct VethInterface {
    pub ip: Ipv4Network,
    pub vlan_id: u16,
}

impl VethInterface {
    pub fn new(ip: Ipv4Addr, vlan_id: u16) -> Self {
        // TODO: make netmask configurable and remove unwrap
        let ip = Ipv4Network::new(ip, 24).unwrap();
        Self { ip, vlan_id }
    }

    pub fn activate(&self) {
        configure_access_port(self.vlan_id, self.ip);
    }

    pub fn get_veth_key(&self) -> VethKey {
        VethKey::new(self.ip.ip(), self.vlan_id)
    }
}
