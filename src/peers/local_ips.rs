use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;

use serde::de::Unexpected;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tun::IntoAddress;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
/// Collection of the relevant local IP addresses.
pub struct LocalIps {
    /// Ethernet IP address of the peer.
    #[serde(deserialize_with = "deserialize_ip", serialize_with = "serialize_ip")]
    pub eth: IpAddr,
    /// TUN IP address of the peer.
    #[serde(deserialize_with = "deserialize_ip", serialize_with = "serialize_ip")]
    pub tun: IpAddr,
    /// Netmask of the peer.
    #[serde(deserialize_with = "deserialize_ip", serialize_with = "serialize_ip")]
    pub netmask: IpAddr,
}

impl LocalIps {
    /// Checks that Ethernet addresses are in the same local network.
    pub fn is_same_ipv4_ethernet_network_of(&self, other: &Self) -> bool {
        if self.netmask != other.netmask
            || !self.netmask.is_ipv4()
            || !self.eth.is_ipv4()
            || !other.eth.is_ipv4()
        {
            return false;
        }

        let netmask = self.netmask.into_address().unwrap().octets();
        let eth_1 = self.eth.into_address().unwrap().octets();
        let eth_2 = other.eth.into_address().unwrap().octets();

        for i in 0..4 {
            if eth_1[i] & netmask[i] != eth_2[i] & netmask[i] {
                return false;
            }
        }

        true
    }
}

fn serialize_ip<S>(ip: &IpAddr, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&ip.to_string())
}

fn deserialize_ip<'de, D>(deserializer: D) -> Result<IpAddr, D::Error>
where
    D: Deserializer<'de>,
{
    let ip_string = String::deserialize(deserializer)?;

    if let Ok(ipv4) = Ipv4Addr::from_str(&ip_string) {
        Ok(IpAddr::V4(ipv4))
    } else {
        Err(serde::de::Error::invalid_value(
            Unexpected::Str(&ip_string),
            &"Valid IP address",
        ))
    }
}
