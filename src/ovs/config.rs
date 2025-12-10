use crate::ovs::helpers::{configure_access_port, setup_br0};
use crate::peers::peer::VethKey;
use ipnetwork::Ipv4Network;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use nullnet_liberror::{Error, ErrorHandler, Location, location};
use serde::{Deserialize, Serialize};
use std::ops::Sub;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct OvsConfig {
    pub vlans: Vec<OvsVlan>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct OvsVlan {
    pub id: u16,
    pub ports: Vec<Ipv4Network>,
}

impl OvsConfig {
    pub const FILE_PATH: &'static str = "ovs/conf.json";

    pub fn load() -> Result<Self, Error> {
        let ovs_json = std::fs::read_to_string(Self::FILE_PATH).handle_err(location!())?;
        let ovs_conf: OvsConfig = serde_json::from_str(&ovs_json).handle_err(location!())?;
        Ok(ovs_conf)
    }

    pub fn configure_access_ports(&self) {
        setup_br0();
        for vlan in &self.vlans {
            for port in &vlan.ports {
                configure_access_port(vlan.id, port);
            }
        }
    }

    pub fn get_veths(&self) -> Vec<VethKey> {
        self.vlans
            .iter()
            .flat_map(|vlan| vlan.ports.iter().map(|net| VethKey::new(net.ip(), vlan.id)))
            .collect()
    }

    pub async fn watch(veths: &Arc<RwLock<Vec<VethKey>>>) -> Result<(), Error> {
        let mut ovs_directory = PathBuf::from(Self::FILE_PATH);
        ovs_directory.pop();

        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = RecommendedWatcher::new(tx, Config::default()).handle_err(location!())?;
        watcher
            .watch(&ovs_directory, RecursiveMode::Recursive)
            .handle_err(location!())?;

        let mut last_update_time = Instant::now().sub(Duration::from_secs(60));

        loop {
            // only update OVS config if the event is related to a file change
            if let Ok(Ok(Event {
                kind: EventKind::Modify(_),
                ..
            })) = rx.recv()
            {
                // debounce duplicated events
                if last_update_time.elapsed().as_millis() > 100 {
                    // ensure file changes are propagated
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    let ovs_conf = Self::load()?;
                    ovs_conf.configure_access_ports();
                    *veths.write().await = ovs_conf.get_veths();
                    last_update_time = Instant::now();
                }
            }
        }
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
        let json = serde_json::to_string_pretty(&config).unwrap();
        let expected_json = std::fs::read_to_string("test_material/ovs.json").unwrap();
        assert_eq!(expected_json, json);
    }
}
