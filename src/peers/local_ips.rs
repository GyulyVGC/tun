use crate::peers::ethernet_addr::EthernetAddr;

#[derive(Clone, Debug)]
/// Collection of the relevant local IP addresses.
pub struct LocalIps {
    /// Ethernet IP address of the peer.
    pub ethernet: EthernetAddr,
}
