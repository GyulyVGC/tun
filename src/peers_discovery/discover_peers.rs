use crate::PORT;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;

const UNICAST_PORT: u16 = PORT - 1;
const BROADCAST_PORT: u16 = PORT - 2;

const RETRIES: u8 = 4;

// values in seconds
const TTL: u64 = 60 * 60;
const RETRANSMISSION_PERIOD: u64 = TTL / 4;
const RETRIES_DELTA: u64 = 1;

pub async fn discover_peers(local_eth_ip: IpAddr, tun_ip: &IpAddr) {
    let local_socket_addr = SocketAddr::new(local_eth_ip, UNICAST_PORT);
    let local_socket = UdpSocket::bind(local_socket_addr).await.unwrap(); // should not panic...
    local_socket.set_broadcast(true).unwrap();
    let local_socket_shared = Arc::new(local_socket);

    tokio::spawn(async move {
        listen_broadcast().await; // this will also call hello_unicast...
    });
    // also listen_unicast will be spawned here...

    hello_broadcast(local_socket_shared, tun_ip).await;
}

/// Listens to broadcast messages. TODO!
async fn listen_broadcast() {
    let mut msg = [0; 1024];
    let listen_broadcast_socket = UdpSocket::bind(listen_broadcast_socket()).await.unwrap();
    loop {
        let (msg_len, from) = listen_broadcast_socket
            .recv_from(&mut msg)
            .await
            .unwrap_or_else(|_| (0, SocketAddr::from_str("0.0.0.0:0").unwrap()));
        println!(
            "Received: {}\tFrom: {from}",
            std::str::from_utf8(&msg[..msg_len]).unwrap()
        );
    }
}

/// Periodically sends out messages to let other peers know that this device is up.
async fn hello_broadcast(local_socket: Arc<UdpSocket>, tun_ip: &IpAddr) {
    let hello_broadcast_socket = hello_broadcast_socket();
    let tun_ip_string = tun_ip.to_string();
    let msg = tun_ip_string.as_bytes();
    loop {
        for _ in 0..RETRIES {
            let _msg_len = local_socket
                .send_to(msg, hello_broadcast_socket)
                .await
                .unwrap_or(0);
            tokio::time::sleep(Duration::from_secs(RETRIES_DELTA)).await;
        }
        tokio::time::sleep(Duration::from_secs(RETRANSMISSION_PERIOD)).await;
    }
}

/// Returns the broadcast socket destination of greeting messages.
fn hello_broadcast_socket() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), BROADCAST_PORT)
}

/// Returns the socket used to listen to greeting messages.
fn listen_broadcast_socket() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), BROADCAST_PORT)
}
