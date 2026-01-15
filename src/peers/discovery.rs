use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use crate::DISCOVERY_PORT;
use crate::local_endpoints::LocalEndpoints;
use crate::peers::database::{PeerDbData, manage_peers_db};
use crate::peers::hello::Hello;
use crate::peers::local_ips::LocalIps;
use crate::peers::peer::{PeerKey, Peers};
use crate::peers::peer_message::PeerMessage;
use chrono::Utc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{RwLock, mpsc};

/// Number of copies for each of the produced `Hello` messages (each of the copies must have its own timestamp anyway).
const RETRIES: u64 = 4;

/// Time to live before a peer is removed from the local list (seconds).
pub(crate) const TTL: u64 = 60;

/// Retransmission period of broadcast `Hello` messages (seconds).
const RETRANSMISSION_PERIOD: u64 = TTL / 2 - 1;

/// Period between the forward of two consecutive `Hello` message copies (seconds).
const RETRIES_DELTA: u64 = 1;

/// Peers discovery mechanism; it consists of different tasks:
/// - update the peers database
/// - listen for broadcast `Hello` messages
/// - listen for (and produces) unicast `Hello` responses
/// - remove inactive peers when their TTL expires
/// - periodically send out broadcast `Hello` messages
pub async fn discover_peers(endpoints: LocalEndpoints, peers: Arc<RwLock<Peers>>) {
    let socket = endpoints.sockets.discovery;
    let socket_2 = socket.clone();
    let socket_3 = socket.clone();
    let socket_4 = socket.clone();

    let broadcast_socket = endpoints.sockets.discovery_broadcast;

    let local_ips = endpoints.ips;
    let local_ips_2 = local_ips.clone();
    let local_ips_3 = local_ips.clone();

    let peers_2 = peers.clone();
    let peers_3 = peers.clone();

    let (tx, rx) = mpsc::unbounded_channel();
    let tx_2 = tx.clone();
    let tx_3 = tx.clone();

    // update peers database
    tokio::spawn(async move {
        manage_peers_db(rx)
            .await
            .expect("Managing peers database failed");
    });

    // listen for broadcast hello messages
    tokio::spawn(async move {
        listen(broadcast_socket, socket, local_ips, peers, tx).await;
    });

    // listen for unicast hello responses AND unicast VLAN setup requests
    tokio::spawn(async move {
        listen(socket_2, socket_3, local_ips_2, peers_2, tx_2).await;
    });

    // remove inactive peers
    tokio::spawn(async move {
        remove_inactive_peers(peers_3, tx_3).await;
    });

    // periodically send out broadcast hello messages
    greet_broadcast(socket_4, local_ips_3).await;
}

/// Listens to hello messages (unicast or broadcast), and invokes `greet_unicast` when needed.
async fn listen(
    listen_socket: Arc<UdpSocket>,
    unicast_socket: Arc<UdpSocket>,
    local_ips: LocalIps,
    peers: Arc<RwLock<Peers>>,
    tx: UnboundedSender<PeerDbData>,
) {
    let mut buf = [0; 1024];
    loop {
        let Ok((buf_len, from)) = listen_socket.recv_from(&mut buf).await else {
            continue;
        };

        let now = Utc::now();
        let Some(msg) = PeerMessage::from_toml_bytes(buf.get(0..buf_len).unwrap_or_default())
        else {
            println!("Could not parse peer message from {from}");
            continue;
        };

        match msg {
            PeerMessage::Hello(hello) => {
                if !hello.is_valid(&from, &local_ips) {
                    continue;
                }

                let hello_is_unicast = hello.is_unicast;
                let hello_is_setup = hello.is_setup;

                let delay = (now - hello.timestamp)
                    .num_microseconds()
                    .unwrap_or_default();

                let peer_key = PeerKey::from_ip_addr(hello.ethernet.ip);
                let should_respond_to = peers.write().await.entry_peer(peer_key, hello, delay, &tx);

                if let Some(dest_socket_addr) = should_respond_to
                    && !hello_is_unicast
                {
                    let source = unicast_socket.clone();
                    let local_ips = local_ips.clone();
                    tokio::spawn(async move {
                        greet_unicast(source, dest_socket_addr, local_ips, !hello_is_setup).await;
                    });
                }
            }
            PeerMessage::VlanSetupRequest(vlan_setup_request) => {
                println!("Received VLAN setup request from {from}: {vlan_setup_request:?}");
                // TODO: remove OvsConfig file watching, setup br0 at startup only, support multiple VLANs in the same request

                vlan_setup_request.vlan.activate();

                // send broadcast updates
                local_ips
                    // TODO: use Sets over Vecs
                    .veths
                    .write()
                    .await
                    .extend(vlan_setup_request.vlan.get_veths());
                let source = unicast_socket.clone();
                let local_ips = local_ips.clone();
                let dest =
                    SocketAddr::new(IpAddr::V4(local_ips.ethernet.broadcast), DISCOVERY_PORT);
                tokio::spawn(async move {
                    greet(&source, dest, &local_ips, false, true, false).await;
                });
            }
        }
    }
}

/// Checks for peers inactive for longer than `TTL` seconds and removes them from the peers list.
async fn remove_inactive_peers(peers: Arc<RwLock<Peers>>, tx: UnboundedSender<PeerDbData>) {
    loop {
        let oldest_last_seen = peers.read().await.get_oldest_last_seen();
        let sleep_time = if let Some(ols) = oldest_last_seen {
            // TODO: timestamps must be monotonic!
            let since_oldest = (Utc::now() - ols).num_seconds().unsigned_abs();
            TTL.checked_sub(since_oldest).unwrap_or_default()
        } else {
            TTL
        };

        tokio::time::sleep(Duration::from_secs(sleep_time)).await;

        peers.write().await.remove_inactive_peers(&tx);
    }
}

/// Periodically sends out messages to let all other peers know that this device is up.
async fn greet_broadcast(socket: Arc<UdpSocket>, local_ips: LocalIps) {
    // require unicast responses when this peer first joins the network
    let mut is_setup = true;
    let dest = SocketAddr::new(IpAddr::V4(local_ips.ethernet.broadcast), DISCOVERY_PORT);
    loop {
        greet(&socket, dest, &local_ips, is_setup, true, false).await;
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
    greet(&socket, dest, &local_ips, false, should_retry, true).await;
}

/// Sends out replicated hello messages to broadcast or to a specific peer.
pub(crate) async fn greet(
    socket: &Arc<UdpSocket>,
    dest: SocketAddr,
    local_ips: &LocalIps,
    is_setup: bool,
    should_retry: bool,
    is_unicast: bool,
) {
    for _ in 0..if should_retry { RETRIES } else { 1 } {
        socket
            .send_to(
                Hello::with_details(local_ips, is_setup, is_unicast)
                    .await
                    .to_toml_string()
                    .as_bytes(),
                dest,
            )
            .await
            .unwrap_or_default();
        tokio::time::sleep(Duration::from_secs(RETRIES_DELTA)).await;
    }
}
