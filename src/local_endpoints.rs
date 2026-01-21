use crate::FORWARD_PORT;
use crate::peers::ethernet_addr::EthernetAddr;
use nullnet_liberror::Error;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;

/// Struct including local IP addresses and sockets, used to set configurations
/// and to correctly communicate with peers in the same network.
#[derive(Clone)]
pub struct LocalEndpoints {
    pub ethernet: EthernetAddr,
    pub forward_socket: Arc<UdpSocket>,
}

impl LocalEndpoints {
    /// Parses and handles OVS configuration,
    /// tries to discover a local IP, and binds needed UDP sockets, retrying every 10 seconds in case of problems.
    pub async fn setup() -> Result<Self, Error> {
        loop {
            if let Some(ethernet) = EthernetAddr::find_suitable() {
                let ip = ethernet.ip;
                println!("Local IP address found: {ip}");
                let forward_socket_addr = SocketAddr::new(IpAddr::V4(ip), FORWARD_PORT);
                if let Ok(sock) = UdpSocket::bind(forward_socket_addr).await {
                    let forward_socket = Arc::new(sock);
                    println!("Forward socket bound successfully");
                    return Ok(Self {
                        ethernet,
                        forward_socket,
                    });
                }
            }
            println!("Could not bind all needed sockets; will retry again in 10 seconds...");
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }
}
