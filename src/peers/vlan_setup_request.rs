use crate::ovs::config::OvsVlan;
use serde::{Deserialize, Serialize};

/// Struct representing `VlanSetupRequest` messages exchanged in the scope of peers discovery.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct VlanSetupRequest {
    /// OVS configuration to be applied.
    pub vlan: OvsVlan,
}

#[cfg(test)]
mod tests {

    use crate::peers::vlan_setup_request::VlanSetupRequest;
    use ipnetwork::Ipv4Network;
    use serde_test::{Configure, Token, assert_tokens};
    use std::net::Ipv4Addr;

    fn vlan_setup_request_for_tests() -> VlanSetupRequest {
        VlanSetupRequest {
            vlan: crate::ovs::config::OvsVlan {
                id: 10,
                ports: vec![
                    Ipv4Network::new(Ipv4Addr::new(8, 8, 8, 8), 24).unwrap(),
                    Ipv4Network::new(Ipv4Addr::new(16, 16, 16, 16), 8).unwrap(),
                ],
            },
        }
    }

    #[test]
    fn test_serialize_and_deserialize_vlan_setup_request() {
        let vlan_setup_request = vlan_setup_request_for_tests();

        assert_tokens(
            &vlan_setup_request.readable(),
            &[
                Token::Struct {
                    name: "VlanSetupRequest",
                    len: 1,
                },
                Token::Str("vlan"),
                Token::Struct {
                    name: "OvsVlan",
                    len: 2,
                },
                Token::Str("id"),
                Token::U16(10),
                Token::Str("ports"),
                Token::Seq { len: Some(2) },
                Token::Str("8.8.8.8/24"),
                Token::Str("16.16.16.16/8"),
                Token::SeqEnd,
                Token::StructEnd,
                Token::StructEnd,
            ],
        );
    }

    #[test]
    fn test_toml_string_vlan_setup_request() {
        let vlan_setup_request = vlan_setup_request_for_tests();

        assert_eq!(
            toml::to_string(&vlan_setup_request).unwrap(),
            "[vlan]\n\
             id = 10\n\
             ports = [\"8.8.8.8/24\", \"16.16.16.16/8\"]\n"
        );
    }
}
