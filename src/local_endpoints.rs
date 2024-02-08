use crate::peers::local_ips::LocalIps;
use pcap::{Address, Device};
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;

pub const PORT: u16 = 9999;
pub const PORT_DISCOVERY_UNICAST: u16 = PORT - 1;
pub const PORT_DISCOVERY_BROADCAST: u16 = PORT - 2;

/// Struct including local IP addresses and sockets, used to set configurations
/// and to correctly communicate with peers in the same network.
#[derive(Clone)]
pub struct LocalEndpoints {
    pub ips: LocalIps,
    pub sockets: LocalSockets,
}

impl LocalEndpoints {
    /// Tries to discover a local IP and bind needed UDP sockets, retrying every 10 seconds in case of errors.
    pub async fn new() -> Self {
        loop {
            if let Some(address) = get_eth_address() {
                let eth_ip = address.addr;
                let netmask = address.netmask.unwrap();
                let broadcast_ip = address.broadcast_addr.unwrap();
                println!("Local IP address found: {eth_ip}");
                let forward_socket_addr = SocketAddr::new(eth_ip, PORT);
                if let Ok(forward) = UdpSocket::bind(forward_socket_addr).await {
                    let discovery_socket_addr = SocketAddr::new(eth_ip, PORT_DISCOVERY_UNICAST);
                    if let Ok(discovery) = UdpSocket::bind(discovery_socket_addr).await {
                        forward.set_broadcast(true).unwrap();
                        discovery.set_broadcast(true).unwrap();
                        let tun_ip = get_tun_ip(&eth_ip);
                        return Self {
                            ips: LocalIps {
                                eth: eth_ip,
                                tun: tun_ip,
                            },
                            sockets: LocalSockets {
                                forward: Arc::new(forward),
                                discovery: Arc::new(discovery),
                            },
                        };
                    }
                }
            }
            println!("Could not correctly bind a socket; will retry in 10 seconds...");
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }
}

#[derive(Clone)]
pub struct LocalSockets {
    pub forward: Arc<UdpSocket>,
    pub discovery: Arc<UdpSocket>,
}

fn get_eth_address() -> Option<Address> {
    if let Ok(devices) = Device::list() {
        for device in devices {
            let flags = device.flags;
            if flags.is_up() && flags.is_running() && !flags.is_loopback() {
                for address in device.addresses {
                    if matches!(address.addr, IpAddr::V4(_))
                        && address.netmask.is_some()
                        && address.broadcast_addr.is_some()
                    {
                        return Some(address);
                    }
                }
            }
        }
    }
    None
}

/// Returns an IP address for the TUN device (based on the local Ethernet IP and supposing /24 netmask).
fn get_tun_ip(eth_ip: &IpAddr) -> IpAddr {
    let local_eth_ip_string = eth_ip.to_string();
    let host_part = local_eth_ip_string.split('.').last().unwrap();
    let tun_ip_string = ["10.0.0.", host_part].concat();
    IpAddr::from_str(&tun_ip_string).unwrap()
}
