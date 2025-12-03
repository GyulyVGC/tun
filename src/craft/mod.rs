pub mod reject_payloads;
use std::fmt::Write;

/// Converts a MAC address in its hexadecimal form
pub fn mac_from_dec_to_hex(mac_dec: [u8; 6]) -> String {
    let mut mac_hex = String::new();
    for n in &mac_dec {
        let _ = write!(mac_hex, "{n:02x}:");
    }
    mac_hex.pop();
    mac_hex
}
