use std::net::IpAddr;

pub struct LocalIps {
    /// Ethernet IP address of the peer.
    pub eth: IpAddr,
    /// TUN IP address of the peer.
    pub tun: IpAddr,
}
