use std::net::IpAddr;

#[derive(Clone)]
pub struct LocalIps {
    /// Ethernet IP address of the peer.
    pub eth: IpAddr,
    /// TUN IP address of the peer.
    pub tun: IpAddr,
}
