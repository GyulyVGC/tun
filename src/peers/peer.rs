use crate::local_endpoints::{DISCOVERY_PORT, FORWARD_PORT};
use crate::peers::hello::Hello;
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
    /// Average delay of all hello messages received from this peer (microseconds).
    pub(crate) avg_delay: u64,
    /// Timestamp of the last hello message received from this peer.
    pub(crate) last_seen: DateTime<Utc>,
}

impl Peer {
    /// Creates a new peer after receiving a hello message.
    pub fn with_details(delay: i64, hello: &Hello, is_unicast: bool) -> Self {
        Self {
            eth_ip: hello.ips.eth,
            num_seen_unicast: u64::from(is_unicast),
            num_seen_multicast: u64::from(!is_unicast),
            avg_delay: delay.unsigned_abs(), // TODO: timestamps must be monotonic!
            last_seen: hello.timestamp,
        }
    }

    /// Updates this peer after receiving a hello message.
    pub fn refresh(&mut self, delay: i64, hello: &Hello, is_unicast: bool) {
        let tot_seen_prev = self.num_seen_unicast + self.num_seen_multicast;
        self.avg_delay =
            (tot_seen_prev * self.avg_delay + delay.unsigned_abs()) / (tot_seen_prev + 1); // TODO: timestamps must be monotonic!
        self.num_seen_unicast += u64::from(is_unicast);
        self.num_seen_multicast += u64::from(!is_unicast);
        self.last_seen = hello.timestamp;
        self.eth_ip = hello.ips.eth;
    }

    /// Socket address for normal network operations.
    pub fn forward_socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.eth_ip, FORWARD_PORT)
    }

    /// Socket address for discovery.
    pub fn discovery_socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.eth_ip, DISCOVERY_PORT)
    }
}

impl Display for Peer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        #[allow(clippy::cast_precision_loss)]
        let avg_delay_as_seconds = self.avg_delay as f64 / 1_000_000_f64;
        writeln!(
            f,
            "{}\n\
            \t - num_seen_unicast:   {}\n\
            \t - num_seen_multicast: {}\n\
            \t - last_seen:          {}\n\
            \t - avg_delay:          {:.4} s",
            self.eth_ip,
            self.num_seen_unicast,
            self.num_seen_multicast,
            self.last_seen,
            avg_delay_as_seconds
        )
    }
}
