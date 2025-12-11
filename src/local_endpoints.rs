use crate::ovs::config::OvsConfig;
use crate::peers::eth_addr::EthAddr;
use crate::peers::local_ips::LocalIps;
use crate::{DISCOVERY_PORT, FORWARD_PORT};
use nullnet_liberror::{Error, ErrorHandler, Location, location};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::io;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;

/// Struct including local IP addresses and sockets, used to set configurations
/// and to correctly communicate with peers in the same network.
#[derive(Clone)]
pub struct LocalEndpoints {
    pub ips: LocalIps,
    pub sockets: LocalSockets,
}

/// Collection of the relevant local sockets.
#[derive(Clone)]
pub struct LocalSockets {
    pub forward: Arc<UdpSocket>,
    pub discovery: Arc<UdpSocket>,
    pub discovery_broadcast: Arc<UdpSocket>,
}

impl LocalEndpoints {
    /// Parses and handles OVS configuration,
    /// tries to discover a local IP, and binds needed UDP sockets, retrying every 10 seconds in case of problems.
    pub async fn setup() -> Result<Self, Error> {
        let ovs_conf = OvsConfig::load()?;
        ovs_conf.activate();
        let veths = Arc::new(RwLock::new(ovs_conf.get_veths()));

        loop {
            if let Some(eth_addr) = EthAddr::find_suitable() {
                let ip = eth_addr.ip;
                let netmask = eth_addr.netmask;
                let broadcast = eth_addr.broadcast;
                println!("Local IP address found: {ip}");
                let forward_socket_addr = SocketAddr::new(IpAddr::V4(ip), FORWARD_PORT);
                if let Ok(forward) = UdpSocket::bind(forward_socket_addr).await {
                    let forward_shared = Arc::new(forward);
                    println!("Forward socket bound successfully");
                    let discovery_socket_addr = SocketAddr::new(IpAddr::V4(ip), DISCOVERY_PORT);
                    if let Ok(discovery) = UdpSocket::bind(discovery_socket_addr).await {
                        discovery.set_broadcast(true).handle_err(location!())?;
                        let discovery_shared = Arc::new(discovery);
                        println!("Discovery socket bound successfully");
                        if let Ok(discovery_broadcast_shared) =
                            get_discovery_broadcast_shared(broadcast, &discovery_shared).await
                        {
                            println!("Discovery broadcast socket bound successfully");

                            return Ok(Self {
                                ips: LocalIps {
                                    ethernet: ip,
                                    veths,
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

/// Returns the broadcast socket to use for discovery.
#[allow(clippy::unused_async, clippy::no_effect_underscore_binding)]
async fn get_discovery_broadcast_shared(
    _broadcast: Ipv4Addr,
    _discovery_socket: &Arc<UdpSocket>,
) -> io::Result<Arc<UdpSocket>> {
    #[cfg(not(target_os = "windows"))]
    return UdpSocket::bind(SocketAddr::new(IpAddr::V4(_broadcast), DISCOVERY_PORT))
        .await
        .map(Arc::new);

    // on Windows broadcast cannot be bound directly (https://issues.apache.org/jira/browse/HBASE-9961)
    #[cfg(target_os = "windows")]
    return Ok(_discovery_socket.to_owned());
}
