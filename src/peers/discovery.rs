use crate::local_endpoints::LocalEndpoints;
use crate::peers::hello::Hello;
use crate::peers::local_ips::LocalIps;
use crate::peers::peer::Peer;
use chrono::Utc;
use std::collections::HashMap;
use std::io::SeekFrom;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::{AsyncSeekExt, AsyncWriteExt, BufWriter};
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, RwLock};

const RETRIES: u8 = 4;

// values in seconds
const TTL: u64 = 60 * 60;
const RETRANSMISSION_PERIOD: u64 = TTL / 4;
const RETRIES_DELTA: u64 = 1;

pub async fn discover_peers(endpoints: LocalEndpoints, peers: Arc<RwLock<HashMap<IpAddr, Peer>>>) {
    let socket = endpoints.sockets.discovery;
    let socket_2 = socket.clone();
    let socket_3 = socket_2.clone();
    let multicast_socket = endpoints.sockets.discovery_multicast;

    let multicast_socket_addr = multicast_socket.local_addr().unwrap();

    let local_ips = endpoints.ips.clone();
    let local_ips_2 = local_ips.clone();

    let peers_2 = peers.clone();

    let writer = Arc::new(Mutex::new(BufWriter::new(
        File::create("./peers.txt").await.unwrap(),
    )));
    let writer_2 = writer.clone();

    // listen for multicast hello messages
    tokio::spawn(async move {
        listen_multicast(socket_3, multicast_socket, local_ips, peers, writer).await;
    });

    // listen for unicast hello responses
    tokio::spawn(async move {
        listen_unicast(socket, local_ips_2, peers_2, writer_2).await;
    });

    // periodically send out multicast hello messages
    greet_multicast(socket_2, multicast_socket_addr, endpoints.ips).await;
}

/// Listens to multicast hello messages and invokes `greet_unicast` when needed.
async fn listen_multicast(
    socket: Arc<UdpSocket>,
    multicast_socket: Arc<UdpSocket>,
    local_ips: LocalIps,
    peers: Arc<RwLock<HashMap<IpAddr, Peer>>>,
    writer: Arc<Mutex<BufWriter<File>>>,
) {
    let mut msg = [0; 256];
    loop {
        let (msg_len, from) = multicast_socket
            .recv_from(&mut msg)
            .await
            .unwrap_or_else(|_| (0, SocketAddr::from_str("0.0.0.0:0").unwrap()));

        let now = Utc::now();
        let hello = Hello::from_toml_bytes(&msg[0..msg_len]);

        if !hello.is_valid(&from, &local_ips) {
            continue;
        };

        let delay = (now - hello.timestamp).num_microseconds().unwrap();

        peers
            .write()
            .await
            .entry(hello.ips.tun)
            .and_modify(|p| p.refresh_multicast(delay, &hello))
            .or_insert_with(|| {
                let peer = Peer {
                    eth_ip: hello.ips.eth,
                    num_seen_unicast: 0,
                    num_seen_multicast: 1,
                    sum_delays: delay.unsigned_abs(), // TODO: timestamps must be monotonic!
                    last_seen: hello.timestamp,
                };

                let dest_socket_addr = peer.discovery_socket_addr();
                let local_ips_2 = local_ips.clone();
                let socket_2 = socket.clone();

                tokio::spawn(async move {
                    greet_unicast(socket_2, dest_socket_addr, local_ips_2).await;
                });

                peer
            });

        let mut buffer = writer.lock().await;
        buffer.get_mut().set_len(0).await.unwrap();
        buffer.get_mut().seek(SeekFrom::Start(0)).await.unwrap();
        for peer in peers.read().await.values() {
            buffer
                .write_all(format!("{peer}\n").as_bytes())
                .await
                .unwrap();
        }
        buffer.flush().await.unwrap();
    }
}

/// Listens to unicast hello messages.
async fn listen_unicast(
    socket: Arc<UdpSocket>,
    local_ips: LocalIps,
    peers: Arc<RwLock<HashMap<IpAddr, Peer>>>,
    writer: Arc<Mutex<BufWriter<File>>>,
) {
    let mut msg = [0; 256];
    loop {
        let (msg_len, from) = socket
            .recv_from(&mut msg)
            .await
            .unwrap_or_else(|_| (0, SocketAddr::from_str("0.0.0.0:0").unwrap()));

        let now = Utc::now();
        let hello = Hello::from_toml_bytes(&msg[0..msg_len]);

        if !hello.is_valid(&from, &local_ips) {
            continue;
        };

        let delay = (now - hello.timestamp).num_microseconds().unwrap();

        peers
            .write()
            .await
            .entry(hello.ips.tun)
            .and_modify(|p| p.refresh_unicast(delay, &hello))
            .or_insert_with(|| Peer {
                eth_ip: hello.ips.eth,
                num_seen_unicast: 1,
                num_seen_multicast: 0,
                sum_delays: delay.unsigned_abs(), // TODO: timestamps must be monotonic!
                last_seen: hello.timestamp,
            });

        let mut buffer = writer.lock().await;
        buffer.get_mut().set_len(0).await.unwrap();
        buffer.get_mut().seek(SeekFrom::Start(0)).await.unwrap();
        for peer in peers.read().await.values() {
            buffer
                .write_all(format!("{peer}\n").as_bytes())
                .await
                .unwrap();
        }
        buffer.flush().await.unwrap();
    }
}

/// Periodically sends out messages to let all other peers know that this device is up.
async fn greet_multicast(
    socket: Arc<UdpSocket>,
    multicast_socket_addr: SocketAddr,
    local_ips: LocalIps,
) {
    loop {
        for _ in 0..RETRIES {
            socket
                .send_to(
                    Hello::new(&local_ips).to_toml_string().as_bytes(),
                    multicast_socket_addr,
                )
                .await
                .unwrap_or(0);
            tokio::time::sleep(Duration::from_secs(RETRIES_DELTA)).await;
        }
        tokio::time::sleep(Duration::from_secs(RETRANSMISSION_PERIOD)).await;
    }
}

/// Sends out messages to acknowledge a specific peer that this device is up.
async fn greet_unicast(socket: Arc<UdpSocket>, dest_socket_addr: SocketAddr, local_ips: LocalIps) {
    for _ in 0..RETRIES {
        socket
            .send_to(
                Hello::new(&local_ips).to_toml_string().as_bytes(),
                dest_socket_addr,
            )
            .await
            .unwrap_or(0);
        tokio::time::sleep(Duration::from_secs(RETRIES_DELTA)).await;
    }
}
