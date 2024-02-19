#![allow(clippy::module_name_repetitions)]

use std::net::{IpAddr, SocketAddr};

use chrono::{DateTime, Utc};

use crate::peers::hello::Hello;
use crate::{DISCOVERY_PORT, FORWARD_PORT};

/// Struct representing a peer.
pub struct Peer {
    pub(crate) key: PeerKey,
    pub(crate) val: PeerVal,
}

/// Struct identifying a peer.
#[derive(Eq, Hash, PartialEq, Clone, Copy)]
pub struct PeerKey {
    /// TUN IP address of the peer.
    pub(crate) tun_ip: IpAddr,
}

impl PeerKey {
    pub fn from_slice(slice: [u8; 4]) -> Self {
        Self {
            tun_ip: IpAddr::from(slice),
        }
    }

    pub fn from_ip_addr(ip_addr: IpAddr) -> Self {
        Self { tun_ip: ip_addr }
    }
}

/// Struct including attributes of a peer.
#[derive(Clone)]
pub struct PeerVal {
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

impl PeerVal {
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

    /// Returns the average delay of messages from this peer, expressed as seconds.
    #[allow(clippy::cast_precision_loss)]
    pub fn avg_delay_as_seconds(&self) -> f64 {
        self.avg_delay as f64 / 1_000_000_f64
    }
}
