use crate::peers::local_ips::LocalIps;
use pcap::{Address, Device};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;

pub const FORWARD_PORT: u16 = 9999;
pub const DISCOVERY_PORT: u16 = FORWARD_PORT - 1;

const MULTICAST_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(224, 0, 0, 1));

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
                println!("Local IP address found: {eth_ip}");
                let forward_socket_addr = SocketAddr::new(eth_ip, FORWARD_PORT);
                if let Ok(forward) = UdpSocket::bind(forward_socket_addr).await {
                    let discovery_socket_addr = SocketAddr::new(eth_ip, DISCOVERY_PORT);
                    if let Ok(discovery) = UdpSocket::bind(discovery_socket_addr).await {
                        let discovery_multicast_socket_addr =
                            SocketAddr::new(MULTICAST_IP, DISCOVERY_PORT);
                        if let Ok(discovery_multicast) =
                            UdpSocket::bind(discovery_multicast_socket_addr).await
                        {
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
                                    discovery_multicast: Arc::new(discovery_multicast),
                                },
                            };
                        }
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
    pub discovery_multicast: Arc<UdpSocket>,
}

/// Checks all the available network devices and returns IP address and netmask of the "suitable" interface.
///
/// The "suitable" interface satisfies the following:
/// - it's IPv4
/// - it has a netmask
/// - it's up
/// - it's running
/// - it's not loopback
fn get_eth_address() -> Option<Address> {
    if let Ok(devices) = Device::list() {
        for device in devices {
            let flags = device.flags;
            if flags.is_up() && flags.is_running() && !flags.is_loopback() {
                for address in device.addresses {
                    if matches!(address.addr, IpAddr::V4(_))
                        && address.netmask.is_some()
                    {
                        return Some(address);
                    }
                }
            }
        }
    }
    None
}

/// Returns an IP address for the TUN device.
/// TODO: support every kind of netmask.
fn get_tun_ip(eth_ip: &IpAddr) -> IpAddr {
    let local_eth_ip_string = eth_ip.to_string();
    let host_part = local_eth_ip_string.split('.').last().unwrap();
    let tun_ip_string = ["10.0.0.", host_part].concat();
    IpAddr::from_str(&tun_ip_string).unwrap()
}