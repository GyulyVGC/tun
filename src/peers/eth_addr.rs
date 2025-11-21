use std::net::{IpAddr, Ipv4Addr};

pub struct EthAddr {
    pub ip: Ipv4Addr,
    pub netmask: Ipv4Addr,
    pub broadcast: Ipv4Addr,
}

impl EthAddr {
    fn new(ip: Ipv4Addr, netmask: Ipv4Addr, broadcast: Ipv4Addr) -> Self {
        Self {
            ip,
            netmask,
            broadcast,
        }
    }

    fn is_suitable(&self) -> bool {
        !self.netmask.is_unspecified() && !self.broadcast.is_unspecified() && self.ip.is_private()
    }

    /// Checks the available network devices and returns IP address, netmask, and broadcast of the first "suitable" interface.
    ///
    /// A "suitable" interface satisfies the following:
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
    pub fn find_suitable() -> Option<EthAddr> {
        use network_interface::{NetworkInterface, NetworkInterfaceConfig};

        if let Ok(devices) = NetworkInterface::show() {
            for device in devices {
                for address in device.addr {
                    if let Some(IpAddr::V4(netmask)) = address.netmask()
                        && let Some(IpAddr::V4(broadcast)) = address.broadcast()
                    {
                        let IpAddr::V4(ip) = address.ip() else {
                            continue;
                        };
                        let eth_addr = EthAddr::new(ip, netmask, broadcast);
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
    pub fn find_suitable() -> Option<EthAddr> {
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
                                        let eth_addr = EthAddr::new(ip, netmask, broadcast);
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
