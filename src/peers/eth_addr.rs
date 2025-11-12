use crate::peers::local_ips::IntoIpv4;
use std::net::IpAddr;

pub struct EthAddr {
    pub ip: IpAddr,
    pub netmask: IpAddr,
    pub broadcast: IpAddr,
}

impl EthAddr {
    fn new(ip: IpAddr, netmask: IpAddr, broadcast: IpAddr) -> Self {
        Self {
            ip,
            netmask,
            broadcast,
        }
    }

    fn is_suitable(&self) -> bool {
        self.netmask.is_ipv4()
            && !self.netmask.is_unspecified()
            && self.broadcast.is_ipv4()
            && !self.broadcast.is_unspecified()
            && self.ip.is_ipv4()
            && self
                .ip
                .into_ipv4()
                .map(|ip| ip.is_private())
                .unwrap_or(false)
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
                    if let Some(netmask) = address.netmask()
                        && let Some(broadcast) = address.broadcast()
                    {
                        let ip = address.ip();
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
