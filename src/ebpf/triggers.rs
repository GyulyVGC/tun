use serde::Deserialize;
use std::collections::HashMap;

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
