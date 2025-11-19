use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;

use serde::de::Unexpected;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
/// Collection of the relevant local IP addresses.
pub struct LocalIps {
    /// Ethernet IP address of the peer.
    #[serde(deserialize_with = "deserialize_ip", serialize_with = "serialize_ip")]
    pub eth: Ipv4Addr,
    /// TUN IP address of the peer.
    #[serde(deserialize_with = "deserialize_ip", serialize_with = "serialize_ip")]
    pub tun: Ipv4Addr,
    /// Netmask of the peer.
    #[serde(deserialize_with = "deserialize_ip", serialize_with = "serialize_ip")]
    pub netmask: Ipv4Addr,
    /// Broadcast address of the peer.
    #[serde(deserialize_with = "deserialize_ip", serialize_with = "serialize_ip")]
    pub broadcast: Ipv4Addr,
}

impl LocalIps {
    /// Checks that Ethernet addresses are in the same local network.
    pub fn is_same_ipv4_ethernet_network_of(&self, other: &Self) -> bool {
        if self.netmask != other.netmask || self.broadcast != other.broadcast {
            return false;
        }

        let netmask = self.netmask.octets();
        let eth_1 = self.eth.octets();
        let eth_2 = other.eth.octets();

        for i in 0..4 {
            if eth_1[i] & netmask[i] != eth_2[i] & netmask[i] {
                return false;
            }
        }

        true
    }
}

fn serialize_ip<S>(ip: &Ipv4Addr, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&ip.to_string())
}

fn deserialize_ip<'de, D>(deserializer: D) -> Result<Ipv4Addr, D::Error>
where
    D: Deserializer<'de>,
{
    let ip_string = String::deserialize(deserializer)?;

    Ipv4Addr::from_str(&ip_string).map_err(|_| {
        serde::de::Error::invalid_value(Unexpected::Str(&ip_string), &"Valid IP address")
    })
}

pub trait IntoIpv4 {
    fn into_ipv4(self) -> Option<Ipv4Addr>;
}

impl IntoIpv4 for IpAddr {
    fn into_ipv4(self) -> Option<Ipv4Addr> {
        match self {
            IpAddr::V4(ipv4) => Some(ipv4),
            IpAddr::V6(_) => None,
        }
    }
}
