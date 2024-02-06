// use std::collections::HashMap;
// use std::net::IpAddr;
// use std::net::SocketAddr;
//
// use once_cell::sync::Lazy;
//
// use crate::PORT;
//
// static ETHERNET_TUN_TUPLES: Lazy<Vec<([u8; 4], [u8; 4])>> = Lazy::new(|| {
//     vec![
//         // Proxmox VM 999 (Debian)
//         ([192, 168, 1, 162], [10, 0, 0, 1]),
//         // Proxmox VM 997 (Debian)
//         ([192, 168, 1, 144], [10, 0, 0, 2]),
//         // macOS
//         ([192, 168, 1, 113], [10, 0, 0, 3]),
//         // Proxmox VM 993 (Windows)
//         ([192, 168, 1, 12], [10, 0, 0, 4]),
//         // Proxmox VM 995 (Fedora)
//         ([192, 168, 1, 72], [10, 0, 0, 5]),
//     ]
// });
//
// // The following static map is automatically generated from the peers list
//
// pub static TUN_TO_SOCKET: Lazy<HashMap<[u8; 4], SocketAddr>> = Lazy::new(|| {
//     let mut map = HashMap::new();
//     for (ethernet, tun) in ETHERNET_TUN_TUPLES.iter() {
//         assert!(map
//             .insert(*tun, SocketAddr::new(IpAddr::from(*ethernet), PORT))
//             .is_none());
//     }
//     map
// });
