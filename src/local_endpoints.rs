#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::io;
use tokio::net::UdpSocket;

use crate::peers::eth_addr::EthAddr;
use crate::peers::local_ips::{IntoIpv4, LocalIps};
use crate::{DISCOVERY_PORT, FORWARD_PORT, MULTICAST, NETWORK};

/// Struct including local IP addresses and sockets, used to set configurations
/// and to correctly communicate with peers in the same network.
pub struct LocalEndpoints {
    pub ips: LocalIps,
    pub sockets: LocalSockets,
}

/// Collection of the relevant local sockets.
pub struct LocalSockets {
    pub forward: Arc<UdpSocket>,
    pub discovery: Arc<UdpSocket>,
    pub discovery_multicast: Arc<UdpSocket>,
}

impl LocalEndpoints {
    /// Tries to discover a local IP and bind needed UDP sockets, retrying every 10 seconds in case of problems.
    pub async fn setup() -> Self {
        loop {
            if let Some(eth_addr) = get_eth_addr() {
                let eth_ip = eth_addr.ip;
                let netmask = eth_addr.netmask;
                println!("Local IP address found: {eth_ip}");
                let forward_socket_addr = SocketAddr::new(eth_ip, FORWARD_PORT);
                if let Ok(forward) = UdpSocket::bind(forward_socket_addr).await {
                    let forward_shared = Arc::new(forward);
                    println!("Forward socket bound successfully");
                    let discovery_socket_addr = SocketAddr::new(eth_ip, DISCOVERY_PORT);
                    if let Ok(discovery) = UdpSocket::bind(discovery_socket_addr).await {
                        let discovery_shared = Arc::new(discovery);
                        println!("Discovery socket bound successfully");
                        if let Ok(discovery_multicast_shared) =
                            get_discovery_multicast_shared(&discovery_shared).await
                        {
                            println!("Discovery multicast socket bound successfully");

                            return Self {
                                ips: LocalIps {
                                    eth: eth_ip,
                                    tun: get_tun_ip(&eth_ip, &netmask),
                                    netmask,
                                },
                                sockets: LocalSockets {
                                    forward: forward_shared,
                                    discovery: discovery_shared,
                                    discovery_multicast: discovery_multicast_shared,
                                },
                            };
                        }
                    }
                }
            }
            println!("Could not bind all needed sockets; will retry again in 10 seconds...");
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }
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
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn get_eth_addr() -> Option<EthAddr> {
    if let Ok(devices) = NetworkInterface::show() {
        for device in devices {
            for address in device.addr {
                if let Some(netmask) = address.netmask() {
                    let ip = address.ip();
                    if netmask.is_ipv4()
                        && !netmask.is_unspecified()
                        && ip.is_ipv4()
                        && ip.into_ipv4().unwrap().is_private()
                    {
                        return Some(EthAddr { ip, netmask });
                    }
                }
            }
        }
    }
    None
}

#[cfg(all(
    not(target_os = "linux"),
    not(target_os = "macos"),
    not(target_os = "windows")
))]
fn get_eth_addr() -> Option<EthAddr> {
    if let Ok(devices) = nix::ifaddrs::getifaddrs() {
        for device in devices {
            println!("{:?}", device);
        }
    }
    None
}

/// Returns an IP address for the TUN device.
fn get_tun_ip(eth_ip: &IpAddr, netmask: &IpAddr) -> IpAddr {
    let eth_ip_octets = eth_ip.into_ipv4().unwrap().octets();
    let netmask_octets = netmask.into_ipv4().unwrap().octets();
    let tun_net_octets = NETWORK.into_ipv4().unwrap().octets();
    let mut tun_ip_octets = [0; 4];

    for i in 0..4 {
        tun_ip_octets[i] = tun_net_octets[i] | (eth_ip_octets[i] & !netmask_octets[i]);
    }

    IpAddr::from(tun_ip_octets)
}

/// Returns the multicast socket to use for discovery.
#[allow(clippy::unused_async, clippy::no_effect_underscore_binding)]
async fn get_discovery_multicast_shared(
    _discovery_socket: &Arc<UdpSocket>,
) -> io::Result<Arc<UdpSocket>> {
    #[cfg(not(target_os = "windows"))]
    {
        UdpSocket::bind(SocketAddr::new(MULTICAST, DISCOVERY_PORT))
            .await
            .map(Arc::new)
    }

    // on Windows multicast cannot be bound directly (https://issues.apache.org/jira/browse/HBASE-9961)
    #[cfg(target_os = "windows")]
    {
        _discovery_socket
            .join_multicast_v4(
                MULTICAST.into_ipv4().unwrap(),
                std::net::Ipv4Addr::UNSPECIFIED,
            )
            .unwrap();
        Ok(_discovery_socket.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use super::*;

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
