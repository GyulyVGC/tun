use std::net::IpAddr;

pub struct LocalInfo {
    /// Ethernet IP address of the peer.
    pub eth_ip: IpAddr,
    /// TUN IP address of the peer.
    pub tun_ip: IpAddr,
}
