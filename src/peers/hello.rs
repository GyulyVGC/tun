use crate::peers::ethernet_addr::EthernetAddr;
use crate::peers::local_ips::LocalIps;
use crate::peers::peer::VethKey;
use crate::peers::processes::Processes;
use chrono::{DateTime, Utc};
use serde::de::Unexpected;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::net::SocketAddr;
use std::str::FromStr;

/// Struct representing Hello messages exchanged in the scope of peers discovery.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Hello {
    /// Ethernet IP of the peer sending the message.
    #[serde(flatten)]
    pub ethernet: EthernetAddr,
    /// Veths of the peer sending the message.
    pub veths: Vec<VethKey>,
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
    pub async fn with_details(local_ips: &LocalIps, is_setup: bool, is_unicast: bool) -> Self {
        let veth_ips: Vec<_> = local_ips
            .veths
            .read()
            .await
            .iter()
            .map(|v| v.veth_ip)
            .collect();
        let processes =
            Processes::from_listeners(listeners::get_all().unwrap_or_default(), &veth_ips);
        Self {
            ethernet: local_ips.ethernet,
            veths: local_ips.veths.read().await.clone(),
            timestamp: Utc::now(),
            is_setup,
            is_unicast,
            processes,
        }
    }

    /// Checks the `Hello` message is valid; a message is valid if:
    /// - the Ethernet address specified is consistent with the address sending the message
    /// - the message was not sent from this machine itself
    /// - the Ethernet address is in the same local network of this machine
    pub fn is_valid(
        &self,
        from: &SocketAddr,
        local_ips: &LocalIps,
        // received_at: &DateTime<Utc>, TODO: timestamps must be monotonic!
    ) -> bool {
        // Ethernet address corresponds to sender socket address
        self.ethernet.ip == from.ip()
            // hello was not sent from this machine
            && self.ethernet.ip != local_ips.ethernet.ip
            // are in the same Ethernet IPv4 network
            && local_ips.ethernet.is_same_ipv4_ethernet_network_of(self.ethernet)
        // delay is non negative TODO: timestamps must be monotonic!
        // && received_at >= &self.timestamp
    }

    /// Serializes this message to a TOML string.
    pub fn to_toml_string(&self) -> String {
        toml::to_string(self).unwrap_or_default()
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
    use crate::peers::ethernet_addr::EthernetAddr;
    use crate::peers::hello::Hello;
    use crate::peers::peer::VethKey;
    use crate::peers::processes::Processes;
    use chrono::{DateTime, Utc};
    use listeners::{Listener, Process, Protocol};
    use serde_test::{Configure, Token, assert_tokens};
    use std::collections::HashSet;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::str::FromStr;

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
            &vec![Ipv4Addr::from_str("10.0.0.9").unwrap()],
        )
    }

    fn hello_for_tests(timestamp: DateTime<Utc>) -> Hello {
        Hello {
            ethernet: EthernetAddr::new(
                Ipv4Addr::from_str("8.8.8.8").unwrap(),
                Ipv4Addr::from_str("255.255.255.0").unwrap(),
                Ipv4Addr::from_str("8.8.8.255").unwrap(),
            ),
            veths: vec![VethKey::new(
                Ipv4Addr::from_str("10.11.12.134").unwrap(),
                10,
            )],
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
            &hello.readable(),
            &[
                Token::Map { len: None },
                Token::Str("ip"),
                Token::Str("8.8.8.8"),
                Token::Str("netmask"),
                Token::Str("255.255.255.0"),
                Token::Str("broadcast"),
                Token::Str("8.8.8.255"),
                Token::Str("veths"),
                Token::Seq { len: Some(1) },
                Token::Struct {
                    name: "veth",
                    len: 2,
                },
                Token::Str("veth_ip"),
                Token::Str("10.11.12.134"),
                Token::Str("vlan_id"),
                Token::U16(10),
                Token::StructEnd,
                Token::SeqEnd,
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
            "ip = \"8.8.8.8\"\n\
             netmask = \"255.255.255.0\"\n\
             broadcast = \"8.8.8.255\"\n\
             timestamp = \"2024-02-08 14:26:23.862231 UTC\"\n\
             is_setup = false\n\
             is_unicast = true\n\
             processes = \"[999/nullnetd on 875, 1234/sshd on 22]\"\n\n\
             [[veths]]\n\
             veth_ip = \"10.11.12.134\"\n\
             vlan_id = 10\n"
        );
    }

    #[test]
    fn test_serialize_and_deserialize_hello_message_with_empty_processes() {
        let timestamp = DateTime::from_str(TEST_TIMESTAMP).unwrap();
        let mut hello = hello_for_tests(timestamp);
        hello.processes = Processes::default();

        assert_tokens(
            &hello.readable(),
            &[
                Token::Map { len: None },
                Token::Str("ip"),
                Token::Str("8.8.8.8"),
                Token::Str("netmask"),
                Token::Str("255.255.255.0"),
                Token::Str("broadcast"),
                Token::Str("8.8.8.255"),
                Token::Str("veths"),
                Token::Seq { len: Some(1) },
                Token::Struct {
                    name: "veth",
                    len: 2,
                },
                Token::Str("veth_ip"),
                Token::Str("10.11.12.134"),
                Token::Str("vlan_id"),
                Token::U16(10),
                Token::StructEnd,
                Token::SeqEnd,
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
