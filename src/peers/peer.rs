use crate::peers::local_ips::LocalIps;

/// Struct representing a peer.
pub struct Peer {
    /// Information about this peer.
    local_info: LocalIps,
    /// Number of times a hello message was received from this peer (multicast + unicast).
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
