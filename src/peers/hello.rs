use chrono::{DateTime, Utc};
use serde::de::Unexpected;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;

/// Struct representing the content of messages exchanged in the scope of peers discovery.
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Hello {
    /// Ethernet IP address of the peer.
    #[serde(deserialize_with = "deserialize_ip", serialize_with = "serialize_ip")]
    eth_ip: IpAddr,
    /// TUN IP address of the peer.
    #[serde(deserialize_with = "deserialize_ip", serialize_with = "serialize_ip")]
    tun_ip: IpAddr,
    /// Timestamp of the message.
    #[serde(
        deserialize_with = "deserialize_timestamp",
        serialize_with = "serialize_timestamp"
    )]
    timestamp: DateTime<Utc>,
}

impl Hello {
    pub fn new(eth_ip: &IpAddr, tun_ip: &IpAddr) -> Self {
        Self {
            eth_ip: eth_ip.to_owned(),
            tun_ip: tun_ip.to_owned(),
            timestamp: Utc::now(),
        }
    }

    pub fn to_toml_string(&self) -> String {
        toml::to_string(self).unwrap()
    }

    pub fn from_toml_bytes(msg: &[u8]) -> Self {
        toml::from_str(std::str::from_utf8(msg).unwrap()).unwrap()
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

fn serialize_timestamp<S>(timestamp: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&timestamp.to_string())
}

fn deserialize_timestamp<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let timestamp_string = String::deserialize(deserializer)?;

    if let Ok(timestamp) = DateTime::from_str(&timestamp_string) {
        Ok(timestamp)
    } else {
        Err(serde::de::Error::invalid_value(
            Unexpected::Str(&timestamp_string),
            &"Valid UTC timestamp",
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::peers::hello::Hello;
    use chrono::{DateTime, Utc};
    use serde_test::{assert_tokens, Token};
    use std::net::IpAddr;
    use std::str::FromStr;

    pub static TEST_TIMESTAMP: &str = "2024-02-08 14:26:23.862231 UTC";

    #[test]
    fn test_serialize_and_deserialize_hello_message() {
        let timestamp = DateTime::from_str(TEST_TIMESTAMP).unwrap();
        let hello = Hello {
            eth_ip: IpAddr::from_str("8.8.8.8").unwrap(),
            tun_ip: IpAddr::from_str("10.11.12.134").unwrap(),
            timestamp,
        };

        assert_tokens(
            &hello,
            &[
                Token::Struct {
                    name: "Hello",
                    len: 3,
                },
                Token::Str("eth_ip"),
                Token::Str("8.8.8.8"),
                Token::Str("tun_ip"),
                Token::Str("10.11.12.134"),
                Token::Str("timestamp"),
                Token::Str(TEST_TIMESTAMP),
                Token::StructEnd,
            ],
        );
    }
}
