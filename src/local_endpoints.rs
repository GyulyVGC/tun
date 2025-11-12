use nullnet_liberror::{Error, ErrorHandler, Location, location};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::io;
use tokio::net::UdpSocket;

use crate::peers::eth_addr::EthAddr;
use crate::peers::local_ips::{IntoIpv4, LocalIps};
use crate::{DISCOVERY_PORT, FORWARD_PORT, NETWORK};

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
    pub discovery_broadcast: Arc<UdpSocket>,
}

impl LocalEndpoints {
    /// Tries to discover a local IP and bind needed UDP sockets, retrying every 10 seconds in case of problems.
    pub async fn setup() -> Result<Self, Error> {
        loop {
            if let Some(eth_addr) = EthAddr::find_suitable() {
                let ip = eth_addr.ip;
                let netmask = eth_addr.netmask;
                let broadcast = eth_addr.broadcast;
                println!("Local IP address found: {ip}");
                let forward_socket_addr = SocketAddr::new(ip, FORWARD_PORT);
                if let Ok(forward) = UdpSocket::bind(forward_socket_addr).await {
                    let forward_shared = Arc::new(forward);
                    println!("Forward socket bound successfully");
                    let discovery_socket_addr = SocketAddr::new(ip, DISCOVERY_PORT);
                    if let Ok(discovery) = UdpSocket::bind(discovery_socket_addr).await {
                        discovery.set_broadcast(true).handle_err(location!())?;
                        let discovery_shared = Arc::new(discovery);
                        println!("Discovery socket bound successfully");
                        if let Ok(discovery_broadcast_shared) =
                            get_discovery_broadcast_shared(broadcast, &discovery_shared).await
                        {
                            println!("Discovery broadcast socket bound successfully");

                            let Some(tun) = get_tun_ip(&ip, &netmask) else {
                                return Err(
                                    "Could not compute TUN IP address from Ethernet IP and netmask",
                                )
                                .handle_err(location!());
                            };

                            return Ok(Self {
                                ips: LocalIps {
                                    eth: ip,
                                    tun,
                                    netmask,
                                    broadcast,
                                },
                                sockets: LocalSockets {
                                    forward: forward_shared,
                                    discovery: discovery_shared,
                                    discovery_broadcast: discovery_broadcast_shared,
                                },
                            });
                        }
                    }
                }
            }
            println!("Could not bind all needed sockets; will retry again in 10 seconds...");
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }
}

/// Returns an IP address for the TUN device.
fn get_tun_ip(eth_ip: &IpAddr, netmask: &IpAddr) -> Option<IpAddr> {
    let eth_ip_octets = eth_ip.into_ipv4()?.octets();
    let netmask_octets = netmask.into_ipv4()?.octets();
    let tun_net_octets = NETWORK.into_ipv4()?.octets();
    let mut tun_ip_octets = [0; 4];

    for i in 0..4 {
        tun_ip_octets[i] = tun_net_octets[i] | (eth_ip_octets[i] & !netmask_octets[i]);
    }

    Some(IpAddr::from(tun_ip_octets))
}

/// Returns the broadcast socket to use for discovery.
#[allow(clippy::unused_async, clippy::no_effect_underscore_binding)]
async fn get_discovery_broadcast_shared(
    _broadcast: IpAddr,
    _discovery_socket: &Arc<UdpSocket>,
) -> io::Result<Arc<UdpSocket>> {
    #[cfg(not(target_os = "windows"))]
    return UdpSocket::bind(SocketAddr::new(_broadcast, DISCOVERY_PORT))
        .await
        .map(Arc::new);

    // on Windows broadcast cannot be bound directly (https://issues.apache.org/jira/browse/HBASE-9961)
    #[cfg(target_os = "windows")]
    return Ok(_discovery_socket.to_owned());
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use super::*;

    #[test]
    fn test_get_tun_ip_netmask_28() {
        let netmask = IpAddr::from([255, 255, 255, 240]);
        let eth_ip = IpAddr::from([192, 168, 1, 109]);
        assert_eq!(
            get_tun_ip(&eth_ip, &netmask),
            Some(IpAddr::from([10, 0, 0, 13]))
        );
    }

    #[test]
    fn test_get_tun_ip_netmask_24() {
        let netmask = IpAddr::from([255, 255, 255, 0]);
        let eth_ip = IpAddr::from([192, 168, 1, 109]);
        assert_eq!(
            get_tun_ip(&eth_ip, &netmask),
            Some(IpAddr::from([10, 0, 0, 109]))
        );
    }

    #[test]
    fn test_get_tun_ip_netmask_20() {
        let netmask = IpAddr::from([255, 255, 240, 0]);
        let eth_ip = IpAddr::from([192, 168, 1, 109]);
        assert_eq!(
            get_tun_ip(&eth_ip, &netmask),
            Some(IpAddr::from([10, 0, 1, 109]))
        );
    }

    #[test]
    fn test_get_tun_ip_netmask_16() {
        let netmask = IpAddr::from([255, 255, 0, 0]);
        let eth_ip = IpAddr::from([192, 168, 1, 109]);
        assert_eq!(
            get_tun_ip(&eth_ip, &netmask),
            Some(IpAddr::from([10, 0, 1, 109]))
        );
    }

    #[test]
    fn test_get_tun_ip_netmask_8() {
        let netmask = IpAddr::from([255, 0, 0, 0]);
        let eth_ip = IpAddr::from([192, 168, 1, 109]);
        assert_eq!(
            get_tun_ip(&eth_ip, &netmask),
            Some(IpAddr::from([10, 168, 1, 109]))
        );
    }
}
