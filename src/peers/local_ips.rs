use std::net::Ipv4Addr;
use std::str::FromStr;

use crate::peers::peer::VethKey;
use serde::de::Unexpected;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
/// Collection of the relevant local IP addresses.
pub struct LocalIps {
    /// Ethernet IP address of the peer.
    #[serde(deserialize_with = "deserialize_ip", serialize_with = "serialize_ip")]
    pub eth: Ipv4Addr,
    /// Veths of the peer.
    pub veths: Vec<VethKey>,
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

#[allow(clippy::trivially_copy_pass_by_ref)]
pub(crate) fn serialize_ip<S>(ip: &Ipv4Addr, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&ip.to_string())
}

pub(crate) fn deserialize_ip<'de, D>(deserializer: D) -> Result<Ipv4Addr, D::Error>
where
    D: Deserializer<'de>,
{
    let ip_string = String::deserialize(deserializer)?;

    Ipv4Addr::from_str(&ip_string).map_err(|_| {
        serde::de::Error::invalid_value(Unexpected::Str(&ip_string), &"Valid IP address")
    })
}

// pub(crate) fn serialize_ip_vec<S>(v: &Vec<Ipv4Addr>, serializer: S) -> Result<S::Ok, S::Error>
// where
//     S: Serializer,
// {
//     let str = v
//         .iter()
//         .map(|ip| ip.to_string())
//         .collect::<Vec<String>>()
//         .join(",");
//     serializer.serialize_str(&format!("[{str}]"))
// }
//
// pub(crate) fn deserialize_ip_vec<'de, D>(deserializer: D) -> Result<Vec<Ipv4Addr>, D::Error>
// where
//     D: Deserializer<'de>,
// {
//     let ip_vec_string = String::deserialize(deserializer)?;
//     let ips_string = ip_vec_string
//         .strip_prefix('[')
//         .and_then(|s| s.strip_suffix(']'))
//         .ok_or_else(|| {
//             serde::de::Error::invalid_value(
//                 Unexpected::Str(&ip_vec_string),
//                 &"Valid IP address list",
//             )
//         })?;
//     let ips: Result<Vec<Ipv4Addr>, _> = ips_string
//         .split(',')
//         .map(|s| s.trim().parse::<Ipv4Addr>())
//         .collect();
//     ips.map_err(|_| {
//         serde::de::Error::invalid_value(Unexpected::Str(&ips_string), &"Valid IP address list")
//     })
// }
