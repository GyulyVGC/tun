use crate::FORWARD_PORT;
use nullnet_liberror::Error;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;

/// Struct including local IP addresses and sockets, used to set configurations
/// and to correctly communicate with peers in the same network.
#[derive(Clone)]
pub struct LocalEndpoints {
    pub ethernet_ip: Ipv4Addr,
    pub forward_socket: Arc<UdpSocket>,
}

impl LocalEndpoints {
    /// Parses and handles OVS configuration,
    /// tries to discover a local IP, and binds needed UDP sockets, retrying every 10 seconds in case of problems.
    pub async fn setup(// rtnetlink_handle: &RtNetLinkHandle
    ) -> Result<Self, Error> {
        loop {
            if let Some(ethernet_ip) = find_ethernet_ip() {
                println!("Local IP address found: {ethernet_ip}");
                let forward_socket_addr = SocketAddr::new(IpAddr::V4(ethernet_ip), FORWARD_PORT);
                if let Ok(sock) = UdpSocket::bind(forward_socket_addr).await {
                    let forward_socket = Arc::new(sock);
                    println!("Forward socket bound successfully");
                    return Ok(Self {
                        ethernet_ip,
                        forward_socket,
                    });
                }
            }
            println!("Could not bind all needed sockets; will retry again in 10 seconds...");
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }
}

fn find_ethernet_ip() -> Option<Ipv4Addr> {
    // TODO: do this using rtnetlink
    use network_interface::{NetworkInterface, NetworkInterfaceConfig};

    if let Ok(devices) = NetworkInterface::show() {
        for device in devices {
            for address in device.addr {
                if device.name == "ens18"
                    && let IpAddr::V4(ip) = address.ip()
                    && ip.is_private()
                {
                    return Some(ip);
                }
            }
        }
    }
    None
}
