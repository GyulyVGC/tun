use crate::peers::ethernet_addr::EthernetAddr;
use crate::peers::peer::VethKey;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
/// Collection of the relevant local IP addresses.
pub struct LocalIps {
    /// Ethernet IP address of the peer.
    pub ethernet: EthernetAddr,
    /// Veths of the peer.
    pub veths: Arc<RwLock<Vec<VethKey>>>,
}
