use crate::peers::local_ips::LocalIps;
use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tun::IntoAddress;

pub const FORWARD_PORT: u16 = 9999;
pub const DISCOVERY_PORT: u16 = FORWARD_PORT - 1;

pub const MULTICAST_IP: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 1);

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
                let eth_ip = address.ip();
                let netmask = address.netmask().unwrap();
                println!("Local IP address found: {eth_ip}");
                let forward_socket_addr = SocketAddr::new(eth_ip, FORWARD_PORT);
                if let Ok(forward) = UdpSocket::bind(forward_socket_addr).await {
                    let discovery_socket_addr = SocketAddr::new(eth_ip, DISCOVERY_PORT);
                    if let Ok(discovery) = UdpSocket::bind(discovery_socket_addr).await {
                        if discovery
                            .join_multicast_v4(MULTICAST_IP, eth_ip.into_address().unwrap())
                            .is_ok()
                        {
                            forward.set_broadcast(true).unwrap();
                            discovery.set_broadcast(true).unwrap();
                            let tun_ip = get_tun_ip(&eth_ip, &netmask);
                            return Self {
                                ips: LocalIps {
                                    eth: eth_ip,
                                    tun: tun_ip,
                                    netmask,
                                },
                                sockets: LocalSockets {
                                    forward: Arc::new(forward),
                                    discovery: Arc::new(discovery),
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
}

/// Checks the available network devices and returns IP address and netmask of the first "suitable" interface.
///
/// A "suitable" interface satisfies the following:
/// - it has a netmask that:
///   - is IP version 4
///   - is not 0.0.0.0
/// - it has an IP address that:
///   - is IP version 4
///   - is a private address (defined by IETF RFC 1918)
fn get_eth_address() -> Option<Addr> {
    if let Ok(devices) = NetworkInterface::show() {
        for device in devices {
            for address in device.addr {
                if let Some(netmask) = address.netmask() {
                    let ip = address.ip();
                    if netmask.is_ipv4()
                        && !netmask.is_unspecified()
                        && ip.is_ipv4()
                        && ip.into_address().unwrap().is_private()
                    // no need to also check the following because of the is_private() check
                    // && !ip.is_unspecified()
                    // && !ip.is_loopback()
                    // && !ip.is_multicast()
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
pub fn get_tun_ip(eth_ip: &IpAddr, netmask: &IpAddr) -> IpAddr {
    let eth_ip_octets = eth_ip.into_address().unwrap().octets();
    let netmask_octets = netmask.into_address().unwrap().octets();

    let tun_net_octets = [10, 0, 0, 0];
    let mut tun_ip_octets = [0; 4];

    for i in 0..4 {
        tun_ip_octets[i] = tun_net_octets[i] | (eth_ip_octets[i] & !netmask_octets[i]);
    }

    IpAddr::from(tun_ip_octets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    #[test]
    fn test_get_tun_ip_netmask_28() {
        let netmask = IpAddr::from([255, 255, 255, 240]);
        let eth_ip = IpAddr::from([192, 168, 1, 109]);
        assert_eq!(get_tun_ip(&eth_ip, &netmask), IpAddr::from([10, 0, 0, 13]));
    }

    #[test]
    fn test_get_tun_ip_netmask_24() {
        let netmask = IpAddr::from([255, 255, 255, 0]);
        let eth_ip = IpAddr::from([192, 168, 1, 109]);
        assert_eq!(get_tun_ip(&eth_ip, &netmask), IpAddr::from([10, 0, 0, 109]));
    }

    #[test]
    fn test_get_tun_ip_netmask_20() {
        let netmask = IpAddr::from([255, 255, 240, 0]);
        let eth_ip = IpAddr::from([192, 168, 1, 109]);
        assert_eq!(get_tun_ip(&eth_ip, &netmask), IpAddr::from([10, 0, 1, 109]));
    }

    #[test]
    fn test_get_tun_ip_netmask_16() {
        let netmask = IpAddr::from([255, 255, 0, 0]);
        let eth_ip = IpAddr::from([192, 168, 1, 109]);
        assert_eq!(get_tun_ip(&eth_ip, &netmask), IpAddr::from([10, 0, 1, 109]));
    }

    #[test]
    fn test_get_tun_ip_netmask_8() {
        let netmask = IpAddr::from([255, 0, 0, 0]);
        let eth_ip = IpAddr::from([192, 168, 1, 109]);
        assert_eq!(
            get_tun_ip(&eth_ip, &netmask),
            IpAddr::from([10, 168, 1, 109])
        );
    }
}
