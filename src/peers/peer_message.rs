use crate::peers::hello::Hello;
use crate::peers::vlan_setup_request::VlanSetupRequest;
use serde::{Deserialize, Serialize};

/// Possible messages exchanged in the scope of peers discovery.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(untagged)]
pub(super) enum PeerMessage {
    Hello(Hello),
    VlanSetupRequest(VlanSetupRequest),
}

impl PeerMessage {
    /// Deserializes TOML bytes into a `PeerMessage`.
    pub fn from_toml_bytes(msg: &[u8]) -> Option<Self> {
        toml::from_str(std::str::from_utf8(msg).unwrap_or_default()).ok()
    }
}
