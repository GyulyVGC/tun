use std::net::IpAddr;

pub struct EthAddr {
    pub ip: IpAddr,
    pub netmask: IpAddr,
    pub broadcast: IpAddr,
}
