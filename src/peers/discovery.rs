use crate::local_endpoints::{LocalEndpoints, DISCOVERY_PORT};
use crate::peers::hello::Hello;
use chrono::Utc;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;

const RETRIES: u8 = 4;

// values in seconds
const TTL: u64 = 60 * 60;
const RETRANSMISSION_PERIOD: u64 = TTL / 4;
const RETRIES_DELTA: u64 = 1;

pub async fn discover_peers(
    endpoints: LocalEndpoints,
    peers: Arc<RwLock<HashMap<IpAddr, SocketAddr>>>,
) {
    let socket = endpoints.sockets.discovery;
    let socket_2 = socket.clone();
    let broadcast_socket = endpoints.sockets.discovery_broadcast;

    let broadcast_socket_addr = broadcast_socket.local_addr().unwrap();

    // listen for broadcast hello messages
    tokio::spawn(async move {
        listen_broadcast(broadcast_socket).await; // this method also invokes greet_unicast when needed
    });

    // listen for unicast hello responses
    tokio::spawn(async move {
        listen_unicast(socket).await;
    });

    // periodically send out broadcast hello messages
    greet_broadcast(
        socket_2,
        broadcast_socket_addr,
        &endpoints.ips.eth,
        &endpoints.ips.tun,
    )
    .await;
}

/// Listens to broadcast messages. TODO!
async fn listen_broadcast(broadcast_socket: Arc<UdpSocket>) {
    let mut msg = [0; 1024];
    loop {
        let (msg_len, from) = broadcast_socket
            .recv_from(&mut msg)
            .await
            .unwrap_or_else(|_| (0, SocketAddr::from_str("0.0.0.0:0").unwrap()));
        let hello = Hello::from_toml_bytes(&msg[0..msg_len]);
        let delay = (Utc::now() - hello.timestamp).num_microseconds().unwrap();
        println!("\n{}", "-".repeat(40));
        println!(
            "Broadcast Hello received\n\
                    \t- from: {from}\n\
                    \t- message: {hello:?}\n\
                    \t- length: {msg_len}\n\
                    \t- delay: {delay}μs",
        );
        println!("{}\n", "-".repeat(40));
    }
}

/// Listens to unicast messages. TODO!
async fn listen_unicast(socket: Arc<UdpSocket>) {
    let mut msg = [0; 1024];
    loop {
        let (msg_len, from) = socket
            .recv_from(&mut msg)
            .await
            .unwrap_or_else(|_| (0, SocketAddr::from_str("0.0.0.0:0").unwrap()));
        println!(
            "Received: {}\tFrom: {from}",
            std::str::from_utf8(&msg[..msg_len]).unwrap()
        );
    }
}

/// Periodically sends out messages to let all other peers know that this device is up.
async fn greet_broadcast(
    socket: Arc<UdpSocket>,
    broadcast_socket_addr: SocketAddr,
    eth_ip: &IpAddr,
    tun_ip: &IpAddr,
) {
    loop {
        for _ in 0..RETRIES {
            socket
                .send_to(
                    Hello::new(eth_ip, tun_ip).to_toml_string().as_bytes(),
                    broadcast_socket_addr,
                )
                .await
                .unwrap_or(0);
            tokio::time::sleep(Duration::from_secs(RETRIES_DELTA)).await;
        }
        tokio::time::sleep(Duration::from_secs(RETRANSMISSION_PERIOD)).await;
    }
}

/// Sends out messages to acknowledge a specific peer that this device is up.
async fn greet_unicast(
    local_socket: Arc<UdpSocket>,
    destination_ip: IpAddr,
    eth_ip: &IpAddr,
    tun_ip: &IpAddr,
) {
    let dest_socket = SocketAddr::new(destination_ip, DISCOVERY_PORT);
    for _ in 0..RETRIES {
        local_socket
            .send_to(
                Hello::new(eth_ip, tun_ip).to_toml_string().as_bytes(),
                dest_socket,
            )
            .await
            .unwrap_or(0);
        tokio::time::sleep(Duration::from_secs(RETRIES_DELTA)).await;
    }
}
