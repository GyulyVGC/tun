use crate::local_endpoints::{LocalEndpoints, DISCOVERY_PORT};
use crate::peers::hello::Hello;
use crate::peers::local_ips::LocalIps;
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
    let multicast_socket = endpoints.sockets.discovery_multicast;

    let multicast_socket_addr = multicast_socket.local_addr().unwrap();

    let local_ips = endpoints.ips.clone();

    // listen for multicast hello messages
    tokio::spawn(async move {
        listen_multicast(multicast_socket, local_ips).await; // this method also invokes greet_unicast when needed
    });

    // listen for unicast hello responses
    tokio::spawn(async move {
        listen_unicast(socket).await;
    });

    // periodically send out multicast hello messages
    greet_multicast(socket_2, multicast_socket_addr, endpoints.ips).await;
}

/// Listens to multicast messages. TODO!
async fn listen_multicast(multicast_socket: Arc<UdpSocket>, local_ips: LocalIps) {
    let mut msg = [0; 256];
    loop {
        let (msg_len, from) = multicast_socket
            .recv_from(&mut msg)
            .await
            .unwrap_or_else(|_| (0, SocketAddr::from_str("0.0.0.0:0").unwrap()));
        let now = Utc::now();
        let hello = Hello::from_toml_bytes(&msg[0..msg_len]);
        let delay = (now - hello.timestamp).num_microseconds().unwrap();

        if !hello.is_valid(&from, &local_ips, &now) {
            continue;
        };

        println!("\n{}", "-".repeat(40));
        println!(
            "Multicast Hello received\n\
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
    let mut msg = [0; 256];
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
async fn greet_unicast(socket: Arc<UdpSocket>, destination_ip: IpAddr, local_ips: &LocalIps) {
    let dest_socket_addr = SocketAddr::new(destination_ip, DISCOVERY_PORT);
    for _ in 0..RETRIES {
        socket
            .send_to(
                Hello::new(local_ips).to_toml_string().as_bytes(),
                dest_socket_addr,
            )
            .await
            .unwrap_or(0);
        tokio::time::sleep(Duration::from_secs(RETRIES_DELTA)).await;
    }
}
