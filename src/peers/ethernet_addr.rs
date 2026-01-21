use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EthernetAddr {
    pub ip: Ipv4Addr,
    pub netmask: Ipv4Addr,
    pub broadcast: Ipv4Addr,
}

impl EthernetAddr {
    pub fn new(ip: Ipv4Addr, netmask: Ipv4Addr, broadcast: Ipv4Addr) -> Self {
        Self {
            ip,
            netmask,
            broadcast,
        }
    }

    // /// Checks that Ethernet addresses are in the same local network.
    // pub fn is_same_ipv4_ethernet_network_of(&self, other: Self) -> bool {
    //     if self.netmask != other.netmask || self.broadcast != other.broadcast {
    //         return false;
    //     }
    //
    //     let netmask = self.netmask.octets();
    //     let eth_1 = self.ip.octets();
    //     let eth_2 = other.ip.octets();
    //
    //     for i in 0..4 {
    //         if eth_1[i] & netmask[i] != eth_2[i] & netmask[i] {
    //             return false;
    //         }
    //     }
    //
    //     true
    // }

    fn is_suitable(&self) -> bool {
        !self.netmask.is_unspecified() && !self.broadcast.is_unspecified() && self.ip.is_private()
    }

    /// Checks the available network devices and returns IP address, netmask, and broadcast of the first "suitable" interface.
    ///
    /// A "suitable" interface satisfies the following:
    /// - its name does not start with "veth"
    /// - it has a netmask that:
    ///   - is IP version 4
    ///   - is not 0.0.0.0
    /// - it has a broadcast address that:
    ///   - is IP version 4
    ///   - is not 0.0.0.0
    /// - it has an IP address that:
    ///   - is IP version 4
    ///   - is a private address (defined by IETF RFC 1918)
    #[cfg(not(target_os = "freebsd"))]
    pub fn find_suitable() -> Option<EthernetAddr> {
        use network_interface::{NetworkInterface, NetworkInterfaceConfig};

        if let Ok(devices) = NetworkInterface::show() {
            for device in devices {
                for address in device.addr {
                    if !device.name.starts_with("veth")
                        && let Some(IpAddr::V4(netmask)) = address.netmask()
                        && let Some(IpAddr::V4(broadcast)) = address.broadcast()
                    {
                        let IpAddr::V4(ip) = address.ip() else {
                            continue;
                        };
                        let eth_addr = EthernetAddr::new(ip, netmask, broadcast);
                        if eth_addr.is_suitable() {
                            return Some(eth_addr);
                        }
                    }
                }
            }
        }
        None
    }

    #[cfg(target_os = "freebsd")]
    pub fn find_suitable() -> Option<EthernetAddr> {
        if let Ok(addrs) = nix::ifaddrs::getifaddrs() {
            for addr in addrs {
                if let Some(sockaddr_ip) = addr.address {
                    if let Some(addr_ip) = sockaddr_ip.as_sockaddr_in() {
                        let ip = IpAddr::from(addr_ip.ip());
                        if let Some(sockaddr_netmask) = addr.netmask {
                            if let Some(addr_netmask) = sockaddr_netmask.as_sockaddr_in() {
                                let netmask = IpAddr::from(addr_netmask.ip());
                                if let Some(sockaddr_broadcast) = addr.broadcast {
                                    if let Some(addr_broadcast) =
                                        sockaddr_broadcast.as_sockaddr_in()
                                    {
                                        let broadcast = IpAddr::from(addr_broadcast.ip());
                                        let eth_addr = EthernetAddr::new(ip, netmask, broadcast);
                                        if eth_addr.is_suitable() {
                                            return Some(eth_addr);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

impl Default for EthernetAddr {
    fn default() -> Self {
        Self {
            ip: Ipv4Addr::UNSPECIFIED,
            netmask: Ipv4Addr::UNSPECIFIED,
            broadcast: Ipv4Addr::UNSPECIFIED,
        }
    }
}
