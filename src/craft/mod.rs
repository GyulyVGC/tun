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

// /// Updates the ARP table for the TUN interface.
// fn update_arp_table(tun_ip: Ipv4Addr, tun_mac: [u8; 6]) {
//     let tun_mac_str = mac_from_dec_to_hex(tun_mac);
//
//     let Ok(mut child) = Command::new("ip")
//         .args([
//             "neigh",
//             "replace",
//             &tun_ip.to_string(),
//             "lladdr",
//             &tun_mac_str,
//             "dev",
//             "nullnet0",
//         ])
//         .spawn()
//         .handle_err(location!())
//     else {
//         return;
//     };
//
//     child.wait().unwrap_or_default();
// }
