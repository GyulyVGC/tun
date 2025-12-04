use crate::ovs::helpers::{configure_access_port, setup_br0};
use ipnetwork::Ipv4Network;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct OvsConfig {
    pub vlans: Vec<OvsVlan>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct OvsVlan {
    pub id: u16,
    // #[serde(
    //     deserialize_with = "deserialize_ip_net",
    //     serialize_with = "serialize_ip_net"
    // )]
    pub ports: Vec<Ipv4Network>,
}

impl OvsConfig {
    pub fn activate(&self) {
        setup_br0();
        for vlan in &self.vlans {
            for port in &vlan.ports {
                configure_access_port(vlan.id, port);
            }
        }
    }

    pub fn get_veths_ips(&self) -> Vec<Ipv4Addr> {
        self.vlans
            .iter()
            .flat_map(|vlan| vlan.ports.iter().map(|net| net.ip()))
            .collect()
    }
}

// pub(crate) fn serialize_ip_net<S>(v: &Vec<Ipv4Network>, serializer: S) -> Result<S::Ok, S::Error>
// where
//     S: Serializer,
// {
//     let str = v
//         .iter()
//         .map(|net| net.to_string())
//         .collect::<Vec<String>>()
//         .join(",");
//     serializer.serialize_str(&format!("[{str}]"))
// }
//
// pub(crate) fn deserialize_ip_net<'de, D>(deserializer: D) -> Result<Vec<Ipv4Network>, D::Error>
// where
//     D: Deserializer<'de>,
// {
//     let net_vec_string = String::deserialize(deserializer)?;
//     let nets_string = net_vec_string
//         .strip_prefix('[')
//         .and_then(|s| s.strip_suffix(']'))
//         .ok_or_else(|| {
//             serde::de::Error::invalid_value(
//                 Unexpected::Str(&net_vec_string),
//                 &"Valid IP networks list",
//             )
//         })?;
//     let ips: Result<Vec<Ipv4Network>, _> = nets_string
//         .split(',')
//         .map(|s| s.trim().parse::<Ipv4Network>())
//         .collect();
//     ips.map_err(|_| {
//         serde::de::Error::invalid_value(Unexpected::Str(&nets_string), &"Valid IP networks list")
//     })
// }

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
        let json = serde_json::to_string_pretty(&config).unwrap();
        let expected_json = std::fs::read_to_string("test_material/ovs.json").unwrap();
        assert_eq!(expected_json, json);
    }
}
