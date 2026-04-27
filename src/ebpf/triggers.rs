use serde::Deserialize;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::Mutex;

const TRIGGERS_PATH: &str = "triggers.toml";

#[derive(Deserialize)]
struct TriggersToml {
    #[serde(default)]
    triggers: Vec<Trigger>,
}

#[derive(Deserialize)]
struct Trigger {
    port: u16,
    service_name: String,
}

pub fn load() -> HashMap<u16, Vec<String>> {
    let content = match std::fs::read_to_string(TRIGGERS_PATH) {
        Ok(c) => c,
        Err(_) => {
            println!("'{TRIGGERS_PATH}' not found, no triggers will be active");
            return HashMap::new();
        }
    };
    let parsed: TriggersToml = match toml::from_str(&content) {
        Ok(p) => p,
        Err(e) => {
            println!("Failed to parse '{TRIGGERS_PATH}': {e}");
            return HashMap::new();
        }
    };
    let mut map: HashMap<u16, Vec<String>> = HashMap::new();
    for t in parsed.triggers {
        map.entry(t.port).or_default().push(t.service_name);
    }
    map
}

pub fn reverse(port_to_services: &HashMap<u16, Vec<String>>) -> HashMap<String, Vec<u16>> {
    let mut out: HashMap<String, Vec<u16>> = HashMap::new();
    for (port, services) in port_to_services {
        for s in services {
            out.entry(s.clone()).or_default().push(*port);
        }
    }
    out
}

pub enum Lifecycle {
    Pending,
    Active {
        vxlan_id: u32,
        overlay_ip: Ipv4Addr,
        ports: Vec<u16>,
    },
}

pub struct TriggersState {
    /// service name -> ports it should DNAT to
    pub service_to_ports: HashMap<String, Vec<u16>>,
    services: Mutex<HashMap<String, Lifecycle>>,
}

impl TriggersState {
    pub fn new(service_to_ports: HashMap<String, Vec<u16>>) -> Self {
        Self {
            service_to_ports,
            services: Mutex::new(HashMap::new()),
        }
    }

    /// Returns true if the caller should fire backend_trigger; false if already pending or active.
    pub fn try_mark_pending(&self, service_name: &str) -> bool {
        let mut svcs = self.services.lock().unwrap();
        if svcs.contains_key(service_name) {
            return false;
        }
        svcs.insert(service_name.to_string(), Lifecycle::Pending);
        true
    }

    pub fn mark_active(
        &self,
        service_name: &str,
        vxlan_id: u32,
        overlay_ip: Ipv4Addr,
        ports: Vec<u16>,
    ) {
        self.services.lock().unwrap().insert(
            service_name.to_string(),
            Lifecycle::Active {
                vxlan_id,
                overlay_ip,
                ports,
            },
        );
    }

    /// Removes the entry (used when backend_trigger fails so the service can be retried).
    pub fn forget(&self, service_name: &str) {
        self.services.lock().unwrap().remove(service_name);
    }

    /// Looks up an Active entry by vxlan_id and removes it. Returns the overlay_ip and ports
    /// so the caller can tear DNAT down.
    pub fn remove_by_vxlan(&self, vxlan_id: u32) -> Option<(Ipv4Addr, Vec<u16>)> {
        let mut svcs = self.services.lock().unwrap();
        let key = svcs.iter().find_map(|(name, lc)| match lc {
            Lifecycle::Active { vxlan_id: v, .. } if *v == vxlan_id => Some(name.clone()),
            _ => None,
        })?;
        match svcs.remove(&key)? {
            Lifecycle::Active {
                overlay_ip, ports, ..
            } => Some((overlay_ip, ports)),
            Lifecycle::Pending => None,
        }
    }
}
