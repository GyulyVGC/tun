use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::de::Unexpected;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::peers::local_ips::LocalIps;
use crate::peers::processes::Processes;

/// Struct representing the content of messages exchanged in the scope of peers discovery.
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Hello {
    /// MAC address of the peer sending the message.
    pub tun_mac: [u8; 6],
    /// Ethernet IP, TUN IP, and netmask of the peer sending the message.
    #[serde(flatten)]
    pub ips: LocalIps,
    /// Timestamp of the message.
    #[serde(
        deserialize_with = "deserialize_timestamp",
        serialize_with = "serialize_timestamp"
    )]
    pub timestamp: DateTime<Utc>,
    /// Whether this message should be acknowledged with unicast answers.
    pub is_setup: bool,
    /// Whether this message is for a single receiver.
    pub is_unicast: bool,
    /// Processes listening on the peer sending the message.
    pub processes: Processes,
}

impl Hello {
    /// Creates a fresh `Hello` message to be sent out.
    pub fn with_details(
        tun_mac: [u8; 6],
        local_ips: &LocalIps,
        is_setup: bool,
        is_unicast: bool,
    ) -> Self {
        let processes = Processes::from_listeners(
            listeners::get_all().unwrap_or_default(),
            IpAddr::V4(local_ips.tun),
        );
        Self {
            tun_mac,
            ips: local_ips.to_owned(),
            timestamp: Utc::now(),
            is_setup,
            is_unicast,
            processes,
        }
    }

    /// Checks the `Hello` message is valid; a message is valid if:
    /// - the Ethernet address specified is consistent with the address sending the message
    /// - the message was not sent from this machine itself
    /// - the TUN address specified is not the same of the local TUN interface
    /// - the Ethernet address is in the same local network of this machine
    pub fn is_valid(
        &self,
        from: &SocketAddr,
        local_ips: &LocalIps,
        // received_at: &DateTime<Utc>, TODO: timestamps must be monotonic!
    ) -> bool {
        let remote_ips = &self.ips;
        // Ethernet address corresponds to sender socket address
        remote_ips.eth == from.ip()
            // hello was not sent from this machine
            && remote_ips.eth != local_ips.eth
            // has not same TUN address of this machine
            && remote_ips.tun != local_ips.tun
            // are in the same Ethernet IPv4 network
            && remote_ips.is_same_ipv4_ethernet_network_of(local_ips)
        // delay is non negative TODO: timestamps must be monotonic!
        // && received_at >= &self.timestamp
    }

    /// Serializes this message to a TOML string.
    pub fn to_toml_string(&self) -> String {
        toml::to_string(self).unwrap_or_default()
    }

    /// Deserializes TOML bytes into a `Hello` message.
    pub fn from_toml_bytes(msg: &[u8]) -> Self {
        toml::from_str(std::str::from_utf8(msg).unwrap_or_default()).unwrap_or_default()
    }
}

impl Default for Hello {
    fn default() -> Self {
        Self {
            tun_mac: [0; 6],
            ips: LocalIps {
                eth: Ipv4Addr::UNSPECIFIED,
                tun: Ipv4Addr::UNSPECIFIED,
                netmask: Ipv4Addr::UNSPECIFIED,
                broadcast: Ipv4Addr::UNSPECIFIED,
            },
            timestamp: DateTime::default(),
            is_setup: false,
            is_unicast: false,
            processes: Processes::default(),
        }
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
    use std::collections::HashSet;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::str::FromStr;

    use chrono::{DateTime, Utc};
    use listeners::{Listener, Process, Protocol};
    use serde_test::{Token, assert_tokens};

    use crate::peers::hello::Hello;
    use crate::peers::local_ips::LocalIps;
    use crate::peers::processes::Processes;

    pub static TEST_TIMESTAMP: &str = "2024-02-08 14:26:23.862231 UTC";

    fn processes_for_tests() -> Processes {
        Processes::from_listeners(
            HashSet::from([
                Listener {
                    process: Process {
                        pid: 1234,
                        name: "sshd".to_string(),
                        path: String::new(),
                    },
                    socket: SocketAddr::new(IpAddr::from_str("10.0.0.9").unwrap(), 22),
                    protocol: Protocol::TCP,
                },
                Listener {
                    process: Process {
                        pid: 999,
                        name: "nullnetd".to_string(),
                        path: String::new(),
                    },
                    socket: SocketAddr::new(IpAddr::from_str("10.0.0.9").unwrap(), 875),
                    protocol: Protocol::TCP,
                },
            ]),
            IpAddr::from_str("10.0.0.9").unwrap(),
        )
    }

    fn hello_for_tests(timestamp: DateTime<Utc>) -> Hello {
        Hello {
            tun_mac: [0; 6],
            ips: LocalIps {
                eth: Ipv4Addr::from_str("8.8.8.8").unwrap(),
                tun: Ipv4Addr::from_str("10.11.12.134").unwrap(),
                netmask: Ipv4Addr::from_str("255.255.255.0").unwrap(),
                broadcast: Ipv4Addr::from_str("8.8.8.255").unwrap(),
            },
            timestamp,
            is_setup: false,
            is_unicast: true,
            processes: processes_for_tests(),
        }
    }

    #[test]
    fn test_serialize_and_deserialize_hello_message() {
        let timestamp = DateTime::from_str(TEST_TIMESTAMP).unwrap();
        let hello = hello_for_tests(timestamp);

        assert_tokens(
            &hello,
            &[
                Token::Map { len: None },
                Token::Str("tun_mac"),
                Token::Tuple { len: 6 },
                Token::U8(0),
                Token::U8(0),
                Token::U8(0),
                Token::U8(0),
                Token::U8(0),
                Token::U8(0),
                Token::TupleEnd,
                Token::Str("eth"),
                Token::Str("8.8.8.8"),
                Token::Str("tun"),
                Token::Str("10.11.12.134"),
                Token::Str("netmask"),
                Token::Str("255.255.255.0"),
                Token::Str("broadcast"),
                Token::Str("8.8.8.255"),
                Token::Str("timestamp"),
                Token::Str(TEST_TIMESTAMP),
                Token::Str("is_setup"),
                Token::Bool(false),
                Token::Str("is_unicast"),
                Token::Bool(true),
                Token::Str("processes"),
                Token::Str("[999/nullnetd on 875, 1234/sshd on 22]"),
                Token::MapEnd,
            ],
        );
    }

    #[test]
    fn test_toml_string_hello_message() {
        let timestamp = DateTime::from_str(TEST_TIMESTAMP).unwrap();
        let hello = hello_for_tests(timestamp);

        assert_eq!(
            hello.to_toml_string(),
            "tun_mac = [0, 0, 0, 0, 0, 0]\n\
             eth = \"8.8.8.8\"\n\
             tun = \"10.11.12.134\"\n\
             netmask = \"255.255.255.0\"\n\
             broadcast = \"8.8.8.255\"\n\
             timestamp = \"2024-02-08 14:26:23.862231 UTC\"\n\
             is_setup = false\n\
             is_unicast = true\n\
             processes = \"[999/nullnetd on 875, 1234/sshd on 22]\"\n"
        );
    }

    #[test]
    fn test_default_hello_message_not_valid() {
        let default = Hello::default();
        let local_ips = LocalIps {
            eth: Ipv4Addr::from([192, 168, 1, 113]),
            tun: Ipv4Addr::from([10, 0, 0, 113]),
            netmask: Ipv4Addr::from([255, 255, 255, 0]),
            broadcast: Ipv4Addr::from([192, 168, 1, 255]),
        };
        assert!(!default.is_valid(&SocketAddr::new(IpAddr::V4(default.ips.eth), 0), &local_ips));
    }

    #[test]
    fn test_serialize_and_deserialize_hello_message_with_empty_processes() {
        let timestamp = DateTime::from_str(TEST_TIMESTAMP).unwrap();
        let mut hello = hello_for_tests(timestamp);
        hello.processes = Processes::default();

        assert_tokens(
            &hello,
            &[
                Token::Map { len: None },
                Token::Str("tun_mac"),
                Token::Tuple { len: 6 },
                Token::U8(0),
                Token::U8(0),
                Token::U8(0),
                Token::U8(0),
                Token::U8(0),
                Token::U8(0),
                Token::TupleEnd,
                Token::Str("eth"),
                Token::Str("8.8.8.8"),
                Token::Str("tun"),
                Token::Str("10.11.12.134"),
                Token::Str("netmask"),
                Token::Str("255.255.255.0"),
                Token::Str("broadcast"),
                Token::Str("8.8.8.255"),
                Token::Str("timestamp"),
                Token::Str(TEST_TIMESTAMP),
                Token::Str("is_setup"),
                Token::Bool(false),
                Token::Str("is_unicast"),
                Token::Bool(true),
                Token::Str("processes"),
                Token::Str("[]"),
                Token::MapEnd,
            ],
        );
    }
}
