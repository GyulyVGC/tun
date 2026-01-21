use crate::ovs::helpers::{configure_access_port, setup_br0};
use crate::peers::peer::VethKey;
use ipnetwork::Ipv4Network;
use nullnet_liberror::{Error, ErrorHandler, Location, location};
use serde::{Deserialize, Serialize};
use std::ops::Sub;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct OvsConfig {
    pub vlans: Vec<OvsVlan>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct OvsVlan {
    pub id: u16,
    pub ports: Vec<Ipv4Network>,
}

impl OvsConfig {
    pub fn activate(&self) {
        setup_br0();
        for vlan in &self.vlans {
            vlan.activate();
        }
    }

    pub fn get_veths(&self) -> Vec<VethKey> {
        self.vlans.iter().flat_map(OvsVlan::get_veths).collect()
    }
}

impl OvsVlan {
    pub fn activate(&self) {
        for port in &self.ports {
            configure_access_port(self.id, *port);
        }
    }

    pub fn get_veths(&self) -> Vec<VethKey> {
        self.ports
            .iter()
            .map(|net| VethKey::new(net.ip(), self.id))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::ovs::config::{OvsConfig, OvsVlan};
    use ipnetwork::Ipv4Network;
    use std::net::Ipv4Addr;

    #[test]
    fn test_deserialize_ovs_config() {
        let json = std::fs::read_to_string("test_material/ovs.json").unwrap();
        let config: OvsConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(
            OvsConfig {
                vlans: vec![
                    OvsVlan {
                        id: 10,
                        ports: vec![Ipv4Network::new(Ipv4Addr::new(10, 0, 10, 1), 24).unwrap(),],
                    },
                    OvsVlan {
                        id: 20,
                        ports: vec![Ipv4Network::new(Ipv4Addr::new(10, 0, 20, 1), 24).unwrap(),],
                    },
                ],
            },
            config
        );
    }

    #[test]
    fn test_serialize_ovs_config() {
        let config = OvsConfig {
            vlans: vec![
                OvsVlan {
                    id: 10,
                    ports: vec![Ipv4Network::new(Ipv4Addr::new(10, 0, 10, 1), 24).unwrap()],
                },
                OvsVlan {
                    id: 20,
                    ports: vec![Ipv4Network::new(Ipv4Addr::new(10, 0, 20, 1), 24).unwrap()],
                },
            ],
        };
        let json = serde_json::to_string(&config).unwrap();
        let expected_json = std::fs::read_to_string("test_material/ovs.json").unwrap();
        assert_eq!(expected_json, json);
    }
}
