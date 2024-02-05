use crate::PORT;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;

const PORT_FOR_DISCOVERY: u16 = PORT - 1;

pub async fn discover_peers(local_eth_ip: IpAddr, tun_ip: &IpAddr) {
    let socket_addr = SocketAddr::new(local_eth_ip, PORT_FOR_DISCOVERY);
    let socket = UdpSocket::bind(socket_addr).await.unwrap(); // should not panic...
    socket.set_broadcast(true).unwrap();
    let socket_shared = Arc::new(socket);

    let socket_shared_2 = socket_shared.clone();
    tokio::spawn(async move {
        listen(socket_shared_2).await;
    });

    tokio::time::sleep(Duration::from_secs(1)).await;

    send_broadcast(socket_shared, tun_ip).await;
}

async fn listen(socket: Arc<UdpSocket>) {
    let mut msg = [0; 1024];
    loop {
        let (msg_len, socket_src) = socket
            .recv_from(&mut msg)
            .await
            .unwrap_or_else(|_| (0, SocketAddr::from_str("0.0.0.0:0").unwrap()));
        println!(
            "Received:\n\t{:?}\nFrom:\n\t{socket_src}",
            msg[..msg_len].to_ascii_lowercase()
        );
    }
}

async fn send_broadcast(socket: Arc<UdpSocket>, tun_ip: &IpAddr) {
    let tun_ip_string = tun_ip.to_string();
    let msg = tun_ip_string.as_bytes();
    loop {
        let _msg_len = socket
            .send_to(msg, get_broadcast_socket())
            .await
            .unwrap_or(0);
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

fn get_broadcast_socket() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), PORT_FOR_DISCOVERY)
}
