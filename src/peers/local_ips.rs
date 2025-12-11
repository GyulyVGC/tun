use crate::peers::peer::VethKey;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
/// Collection of the relevant local IP addresses.
pub struct LocalIps {
    /// Ethernet IP address of the peer.
    pub ethernet: Ipv4Addr,
    /// Netmask of the peer.
    pub netmask: Ipv4Addr,
    /// Broadcast address of the peer.
    pub broadcast: Ipv4Addr,
    /// Veths of the peer.
    pub veths: Arc<RwLock<Vec<VethKey>>>,
}

impl LocalIps {
    /// Checks that Ethernet addresses are in the same local network.
    pub fn is_same_ipv4_ethernet_network_of(
        &self,
        ethernet: Ipv4Addr,
        netmask: Ipv4Addr,
        broadcast: Ipv4Addr,
    ) -> bool {
        if self.netmask != netmask || self.broadcast != broadcast {
            return false;
        }

        let netmask = self.netmask.octets();
        let eth_1 = self.ethernet.octets();
        let eth_2 = ethernet.octets();

        for i in 0..4 {
            if eth_1[i] & netmask[i] != eth_2[i] & netmask[i] {
                return false;
            }
        }

        true
    }
}
