use serde::de::Unexpected;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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
