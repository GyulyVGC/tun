use crate::local_endpoints::{DISCOVERY_PORT, FORWARD_PORT};
use chrono::{DateTime, Utc};
use std::fmt::{Display, Formatter};
use std::net::{IpAddr, SocketAddr};

/// Struct representing a peer.
pub struct Peer {
    /// Ethernet IP address of this peer.
    pub(crate) eth_ip: IpAddr,
    /// Number unicast hello messages received from this peer.
    pub(crate) num_seen_unicast: u64,
    /// Number multicast hello messages received from this peer.
    pub(crate) num_seen_multicast: u64,
    /// Cumulative delays of all hello messages received from this peer (microseconds).
    pub(crate) sum_delays: u64,
    /// Timestamp of the last hello message received from this peer.
    pub(crate) last_seen: DateTime<Utc>,
}

impl Peer {
    /// Average delay of hello messages received from this peer (seconds).
    pub fn avg_delay_as_seconds(&self) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let micros = (self.sum_delays / (self.num_seen_unicast + self.num_seen_multicast)) as f64;
        micros / 1_000_000.0
    }

    /// Socket address for normal network operations.
    pub fn forward_socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.eth_ip, FORWARD_PORT)
    }

    /// Socket address for discovery.
    pub fn discovery_socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.eth_ip, DISCOVERY_PORT)
    }

    /// Updates this peer after receiving a unicast hello.
    pub fn refresh_unicast(&mut self, delay: i64, last_seen: DateTime<Utc>) {
        self.num_seen_unicast += 1;
        self.sum_delays += delay.unsigned_abs(); // TODO: timestamps must be monotonic!
        self.last_seen = last_seen;
    }

    /// Updates this peer after receiving a multicast hello.
    pub fn refresh_multicast(&mut self, delay: i64, last_seen: DateTime<Utc>) {
        self.num_seen_multicast += 1;
        self.sum_delays += delay.unsigned_abs(); // TODO: timestamps must be monotonic!
        self.last_seen = last_seen;
    }
}

impl Display for Peer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "{}\n\
            \t - num_seen_unicast:   {}\n\
            \t - num_seen_multicast: {}\n\
            \t - last_seen:          {}\n\
            \t - avg_delay:          {} s",
            self.eth_ip,
            self.num_seen_unicast,
            self.num_seen_multicast,
            self.last_seen,
            self.avg_delay_as_seconds()
        )
    }
}
