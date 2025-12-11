#![allow(clippy::module_name_repetitions)]

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::peers::database::PeerDbAction;
use crate::peers::discovery::TTL;
use crate::peers::hello::Hello;
use crate::peers::processes::Processes;
use crate::{DISCOVERY_PORT, FORWARD_PORT};
use chrono::{DateTime, Utc};
use nullnet_liberror::location;
use nullnet_liberror::{ErrorHandler, Location};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

/// Struct containing all peers information.
#[derive(Default)]
pub struct Peers {
    /// All known peers.
    map: HashMap<PeerKey, PeerVal>,
    /// Mapping from veth addresses to Ethernet IPs.
    ips: HashMap<VethKey, PeerKey>,
}

impl Peers {
    pub fn get_socket_by_veth(&self, veth_key: VethKey) -> Option<SocketAddr> {
        self.ips.get(&veth_key).map(|pk| pk.forward_socket_addr())
    }

    pub fn get_oldest_last_seen(&self) -> Option<DateTime<Utc>> {
        self.map
            .values()
            .min_by(|p1, p2| p1.last_seen.cmp(&p2.last_seen))
            .map(|p| p.last_seen)
    }

    /// Handles the entry of a peer in the peers list, updating its attributes if already present,
    /// or inserting it otherwise.
    /// Returns the socket address to which a response should be sent
    /// (used to determine whether an unicast response is required).
    pub fn entry_peer(
        &mut self,
        peer_key: PeerKey,
        hello: Hello,
        delay: i64,
        tx: &UnboundedSender<(Peer, PeerDbAction)>,
    ) -> Option<SocketAddr> {
        // update veth to eth mapping
        self.ips.retain(|_, v| v != &peer_key);
        for veth_key in &hello.veths {
            self.ips.insert(*veth_key, peer_key);
        }

        let mut should_respond_to: Option<SocketAddr> = None;
        self.map
            .entry(peer_key)
            .and_modify(|peer_val| {
                peer_val.refresh(delay, &hello);

                if hello.is_setup {
                    should_respond_to = Some(peer_key.discovery_socket_addr());
                }

                // update peer db
                let _ = tx
                    .send((
                        Peer {
                            key: peer_key,
                            val: peer_val.to_owned(),
                        },
                        PeerDbAction::Modify,
                    ))
                    .handle_err(location!());
            })
            .or_insert_with(|| {
                let peer_val = PeerVal::with_details(delay, hello);

                should_respond_to = Some(peer_key.discovery_socket_addr());

                // update peer db
                let _ = tx
                    .send((
                        Peer {
                            key: peer_key,
                            val: peer_val.clone(),
                        },
                        PeerDbAction::Insert,
                    ))
                    .handle_err(location!());

                peer_val
            });

        should_respond_to
    }

    /// Removes peers inactive for longer than `TTL` seconds.
    pub fn remove_inactive_peers(&mut self, tx: &UnboundedSender<(Peer, PeerDbAction)>) {
        self.map.retain(|peer_key, peer_val| {
            let retain = (Utc::now() - peer_val.last_seen)
                .num_seconds()
                .unsigned_abs()
                < TTL;

            // update peer veth mapping and db
            if !retain {
                self.ips.retain(|_, v| v != peer_key);

                let _ = tx
                    .send((
                        Peer {
                            key: *peer_key,
                            val: peer_val.to_owned(),
                        },
                        PeerDbAction::Remove,
                    ))
                    .handle_err(location!());
            }

            retain
        });
    }
}

/// Struct representing a peer.
pub struct Peer {
    pub(crate) key: PeerKey,
    pub(crate) val: PeerVal,
}

/// Struct identifying a peer.
#[derive(Eq, Hash, PartialEq, Clone, Copy)]
pub struct PeerKey {
    /// Ethernet IP address of the peer.
    pub(crate) ethernet_ip: Ipv4Addr,
}

impl PeerKey {
    pub fn from_ip_addr(ip_addr: Ipv4Addr) -> Self {
        Self {
            ethernet_ip: ip_addr,
        }
    }

    /// Socket address for normal network operations.
    pub fn forward_socket_addr(self) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(self.ethernet_ip), FORWARD_PORT)
    }

    /// Socket address for discovery.
    pub fn discovery_socket_addr(self) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(self.ethernet_ip), DISCOVERY_PORT)
    }
}

/// Struct identifying veth on a VLAN.
#[derive(Eq, Hash, PartialEq, Clone, Copy, Serialize, Deserialize, Debug)]
#[serde(rename = "veth")]
pub struct VethKey {
    /// IP address of the veth.
    pub(crate) veth_ip: Ipv4Addr,
    /// VLAN ID of the veth.
    pub(crate) vlan_id: u16,
}

impl VethKey {
    pub fn new(veth_ip: Ipv4Addr, vlan_id: u16) -> Self {
        Self { veth_ip, vlan_id }
    }
}

/// Struct including relevant attributes of a peer.
#[derive(Clone)]
pub struct PeerVal {
    /// veths IP addresses of this peer.
    pub(crate) veths: Vec<VethKey>,
    /// Number of unicast hello messages received from this peer.
    pub(crate) num_seen_unicast: u64,
    /// Number of broadcast hello messages received from this peer.
    pub(crate) num_seen_broadcast: u64,
    /// Average delay of all hello messages received from this peer (microseconds).
    pub(crate) avg_delay: u64,
    /// Timestamp of the last hello message received from this peer.
    pub(crate) last_seen: DateTime<Utc>,
    /// Names of the processes running on this peer.
    pub(crate) processes: Processes,
}

impl PeerVal {
    /// Creates new peer attributes from a `Hello` message.
    pub fn with_details(delay: i64, hello: Hello) -> Self {
        Self {
            veths: hello.veths,
            num_seen_unicast: u64::from(hello.is_unicast),
            num_seen_broadcast: u64::from(!hello.is_unicast),
            avg_delay: delay.unsigned_abs(), // TODO: timestamps must be monotonic!
            last_seen: hello.timestamp,
            processes: hello.processes,
        }
    }

    /// Updates this peer attributes after receiving a `Hello` message.
    pub fn refresh(&mut self, delay: i64, hello: &Hello) {
        let tot_seen_prev = self.num_seen_unicast + self.num_seen_broadcast;
        self.avg_delay =
            (tot_seen_prev * self.avg_delay + delay.unsigned_abs()) / (tot_seen_prev + 1); // TODO: timestamps must be monotonic!
        self.num_seen_unicast += u64::from(hello.is_unicast);
        self.num_seen_broadcast += u64::from(!hello.is_unicast);

        self.veths.clone_from(&hello.veths);
        self.last_seen = hello.timestamp;
        self.processes = hello.processes.clone();
    }

    /// Returns the average delay of messages from this peer, expressed as seconds.
    #[allow(clippy::cast_precision_loss)]
    pub fn avg_delay_as_seconds(&self) -> f64 {
        self.avg_delay as f64 / 1_000_000_f64
    }
}
