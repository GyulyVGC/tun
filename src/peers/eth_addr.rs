use crate::peers::local_ips::IntoIpv4;
use std::net::IpAddr;

pub struct EthAddr {
    pub ip: IpAddr,
    pub netmask: IpAddr,
    pub broadcast: IpAddr,
}

impl EthAddr {
    pub fn new(ip: IpAddr, netmask: IpAddr, broadcast: IpAddr) -> Self {
        Self {
            ip,
            netmask,
            broadcast,
        }
    }

    pub fn is_suitable(&self) -> bool {
        self.netmask.is_ipv4()
            && !self.netmask.is_unspecified()
            && self.broadcast.is_ipv4()
            && !self.broadcast.is_unspecified()
            && self.ip.is_ipv4()
            && self.ip.into_ipv4().unwrap().is_private()
    }
}
