use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use crate::{DISCOVERY_PORT, MULTICAST};
use chrono::Utc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{mpsc, RwLock};

use crate::local_endpoints::LocalEndpoints;
use crate::peers::database::{manage_peers_db, PeerDbAction};
use crate::peers::hello::Hello;
use crate::peers::local_ips::LocalIps;
use crate::peers::peer::{Peer, PeerKey, PeerVal};

/// Number of copies for each of the produced `Hello` messages (each of the copies must have its own timestamp anyway).
const RETRIES: u64 = 4;

/// Time to live before a peer is removed from the local list (seconds).
const TTL: u64 = 60; // * 60;

/// Retransmission period of multicast `Hello` messages (seconds).
const RETRANSMISSION_PERIOD: u64 = TTL / 4;

/// Period between the forward of two consecutive `Hello` message copies (seconds).
const RETRIES_DELTA: u64 = 1;

/// Peers discovery mechanism; it consists of different tasks:
/// - update the peers database
/// - listen for multicast `Hello` messages
/// - listen for (and produces) unicast `Hello` responses
/// - remove inactive peers when their TTL expires
/// - periodically send out multicast `Hello` messages
pub async fn discover_peers(
    endpoints: LocalEndpoints,
    peers: Arc<RwLock<HashMap<PeerKey, PeerVal>>>,
) {
    let socket = endpoints.sockets.discovery;
    let socket_2 = socket.clone();
    let socket_3 = socket.clone();
    let socket_4 = socket.clone();

    let multicast_socket = endpoints.sockets.discovery_multicast;

    let local_ips = endpoints.ips;

    let peers_2 = peers.clone();
    let peers_3 = peers.clone();

    let (tx, rx) = mpsc::unbounded_channel();
    let tx_2 = tx.clone();
    let tx_3 = tx.clone();

    // update peers database
    tokio::spawn(async move {
        manage_peers_db(rx).await;
    });

    // listen for multicast hello messages
    tokio::spawn(async move {
        listen(multicast_socket, socket, local_ips, peers, tx).await;
    });

    // listen for unicast hello responses
    tokio::spawn(async move {
        listen(socket_2, socket_3, local_ips, peers_2, tx_2).await;
    });

    // remove inactive peers
    tokio::spawn(async move {
        remove_inactive_peers(peers_3, tx_3).await;
    });

    // periodically send out multicast hello messages
    greet_multicast(socket_4, local_ips).await;
}

/// Listens to hello messages (unicast or multicast), and invokes `greet_unicast` when needed.
async fn listen(
    listen_socket: Arc<UdpSocket>,
    unicast_socket: Arc<UdpSocket>,
    local_ips: LocalIps,
    peers: Arc<RwLock<HashMap<PeerKey, PeerVal>>>,
    tx: UnboundedSender<(Peer, PeerDbAction)>,
) {
    // used to determine whether an unicast response is required
    let mut should_respond_to;
    let mut msg = [0; 1024];
    loop {
        should_respond_to = None;

        let (msg_len, from) = listen_socket
            .recv_from(&mut msg)
            .await
            .unwrap_or_else(|_| (0, SocketAddr::from_str("0.0.0.0:0").unwrap()));

        let now = Utc::now();
        let hello = Hello::from_toml_bytes(&msg[0..msg_len]);

        if !hello.is_valid(&from, &local_ips) {
            continue;
        };

        let delay = (now - hello.timestamp)
            .num_microseconds()
            .unwrap_or_default();

        let peer_key = PeerKey::from_ip_addr(hello.ips.tun);
        peers
            .write()
            .await
            .entry(peer_key)
            .and_modify(|peer_val| {
                peer_val.refresh(delay, &hello);

                if hello.is_setup {
                    should_respond_to = Some(peer_val.discovery_socket_addr());
                }

                // update peer db
                tx.send((
                    Peer {
                        key: peer_key,
                        val: peer_val.to_owned(),
                    },
                    PeerDbAction::Modify,
                ))
                .unwrap();
            })
            .or_insert_with(|| {
                let peer_val = PeerVal::with_details(delay, &hello);

                should_respond_to = Some(peer_val.discovery_socket_addr());

                // update peer db
                tx.send((
                    Peer {
                        key: peer_key,
                        val: peer_val.clone(),
                    },
                    PeerDbAction::Insert,
                ))
                .unwrap();

                peer_val
            });

        if let Some(dest_socket_addr) = should_respond_to {
            if !hello.is_unicast {
                let source = unicast_socket.clone();
                tokio::spawn(async move {
                    greet_unicast(source, dest_socket_addr, local_ips, !hello.is_setup).await;
                });
            }
        }
    }
}

/// Checks for peers inactive for longer than `TTL` seconds and removes them from the peers list.
async fn remove_inactive_peers(
    peers: Arc<RwLock<HashMap<PeerKey, PeerVal>>>,
    tx: UnboundedSender<(Peer, PeerDbAction)>,
) {
    loop {
        let oldest_peer_val = peers
            .read()
            .await
            .values()
            .min_by(|p1, p2| p1.last_seen.cmp(&p2.last_seen))
            .cloned();
        let sleep_time = if let Some(p) = oldest_peer_val {
            // TODO: timestamps must be monotonic!
            let since_oldest = (Utc::now() - p.last_seen).num_seconds().unsigned_abs();
            TTL.checked_sub(since_oldest).unwrap_or_default()
        } else {
            TTL
        };

        tokio::time::sleep(Duration::from_secs(sleep_time)).await;

        peers.write().await.retain(|peer_key, peer_val| {
            let retain = (Utc::now() - peer_val.last_seen)
                .num_seconds()
                .unsigned_abs()
                < TTL;

            // update peer db
            if !retain {
                tx.send((
                    Peer {
                        key: *peer_key,
                        val: peer_val.to_owned(),
                    },
                    PeerDbAction::Remove,
                ))
                .unwrap();
            }

            retain
        });
    }
}

/// Periodically sends out messages to let all other peers know that this device is up.
async fn greet_multicast(socket: Arc<UdpSocket>, local_ips: LocalIps) {
    // require unicast responses when this peer first joins the network
    let mut is_setup = true;
    let dest = SocketAddr::new(MULTICAST, DISCOVERY_PORT);
    loop {
        greet(&socket, dest, &local_ips, is_setup, true).await;
        is_setup = false;
        tokio::time::sleep(Duration::from_secs(RETRANSMISSION_PERIOD)).await;
    }
}

/// Sends out messages to acknowledge a specific peer that this device is up.
async fn greet_unicast(
    socket: Arc<UdpSocket>,
    dest: SocketAddr,
    local_ips: LocalIps,
    should_retry: bool,
) {
    greet(&socket, dest, &local_ips, false, should_retry).await;
}

/// Sends out replicated hello messages to multicast or to a specific peer.
async fn greet(
    socket: &Arc<UdpSocket>,
    dest: SocketAddr,
    local_ips: &LocalIps,
    is_setup: bool,
    should_retry: bool,
) {
    for _ in 0..if should_retry { RETRIES } else { 1 } {
        socket
            .send_to(
                Hello::with_details(local_ips, is_setup, !dest.ip().is_multicast())
                    .to_toml_string()
                    .as_bytes(),
                dest,
            )
            .await
            .unwrap_or_default();
        tokio::time::sleep(Duration::from_secs(RETRIES_DELTA)).await;
    }
}
