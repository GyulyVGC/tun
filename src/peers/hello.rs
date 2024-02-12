use crate::local_endpoints::get_tun_ip;
use crate::peers::local_ips::LocalIps;
use chrono::{DateTime, Utc};
use serde::de::Unexpected;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::net::SocketAddr;
use std::str::FromStr;

/// Struct representing the content of messages exchanged in the scope of peers discovery.
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Hello {
    /// Ethernet IP, TUN IP, and netmask of the peer.
    pub ips: LocalIps,
    /// Timestamp of the message.
    #[serde(
        deserialize_with = "deserialize_timestamp",
        serialize_with = "serialize_timestamp"
    )]
    pub timestamp: DateTime<Utc>,
}

impl Hello {
    pub fn new(local_ips: &LocalIps) -> Self {
        Self {
            ips: local_ips.to_owned(),
            timestamp: Utc::now(),
        }
    }

    pub fn is_valid(
        &self,
        from: &SocketAddr,
        local_ips: &LocalIps,
        received_at: &DateTime<Utc>,
    ) -> bool {
        let remote_ips = &self.ips;
        remote_ips.eth == from.ip()
            && remote_ips.tun != local_ips.tun
            && remote_ips.netmask == local_ips.netmask
            && remote_ips.tun == get_tun_ip(&remote_ips.eth, &remote_ips.netmask)
            // && received_at >= &self.timestamp
    }

    pub fn to_toml_string(&self) -> String {
        toml::to_string(self).unwrap()
    }

    pub fn from_toml_bytes(msg: &[u8]) -> Self {
        toml::from_str(std::str::from_utf8(msg).unwrap()).unwrap()
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
    use crate::peers::local_ips::LocalIps;
    use chrono::DateTime;
    use serde_test::{assert_tokens, Token};
    use std::net::IpAddr;
    use std::str::FromStr;

    pub static TEST_TIMESTAMP: &str = "2024-02-08 14:26:23.862231 UTC";

    #[test]
    fn test_serialize_and_deserialize_hello_message() {
        let timestamp = DateTime::from_str(TEST_TIMESTAMP).unwrap();
        let hello = Hello {
            ips: LocalIps {
                eth: IpAddr::from_str("8.8.8.8").unwrap(),
                tun: IpAddr::from_str("10.11.12.134").unwrap(),
                netmask: IpAddr::from_str("255.255.255.0").unwrap(),
            },
            timestamp,
        };

        assert_tokens(
            &hello,
            &[
                Token::Struct {
                    name: "Hello",
                    len: 2,
                },
                Token::Str("ips"),
                Token::Struct {
                    name: "LocalIps",
                    len: 3,
                },
                Token::Str("eth"),
                Token::Str("8.8.8.8"),
                Token::Str("tun"),
                Token::Str("10.11.12.134"),
                Token::Str("netmask"),
                Token::Str("255.255.255.0"),
                Token::StructEnd,
                Token::Str("timestamp"),
                Token::Str(TEST_TIMESTAMP),
                Token::StructEnd,
            ],
        );
    }
}
