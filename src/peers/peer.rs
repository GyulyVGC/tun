use std::net::IpAddr;

/// Struct representing a peer.
pub struct Peer {
    /// Ethernet IP address of the peer.
    eth_ip: IpAddr,
    /// TUN IP address of the peer.
    tun_ip: IpAddr,
    /// Number of times a hello message was received from this peer (broadcast + unicast).
    num_seen: u64,
    /// Cumulative delays of all hello messages received from this peer (microseconds).
    sum_delays: u64,
}

impl Peer {
    /// Average delay of hello messages received from this peer (microseconds).
    pub fn avg_delay(&self) -> u64 {
        self.sum_delays / self.num_seen
    }
}
