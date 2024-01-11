use crate::PORT;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::net::IpAddr;
use std::net::SocketAddr;

static ETHERNET_TUN_TUPLES: Lazy<Vec<([u8; 4], [u8; 4])>> = Lazy::new(|| {
    vec![
        // Proxmox VM 999
        ([192, 168, 1, 162], [10, 0, 0, 1]),
        // Proxmox VM 997
        ([192, 168, 1, 144], [10, 0, 0, 2]),
        // macOS
        ([192, 168, 1, 113], [10, 0, 0, 3]),
    ]
});

pub static ETHERNET_TO_TUN: Lazy<HashMap<IpAddr, IpAddr>> = Lazy::new(|| {
    let mut map = HashMap::new();
    for (ethernet, tun) in ETHERNET_TUN_TUPLES.iter() {
        assert!(map
            .insert(IpAddr::from(*ethernet), IpAddr::from(*tun))
            .is_none());
    }
    map
});

pub static TUN_TO_SOCKET: Lazy<HashMap<[u8; 4], SocketAddr>> = Lazy::new(|| {
    let mut map = HashMap::new();
    for (ethernet, tun) in ETHERNET_TUN_TUPLES.iter() {
        assert!(map
            .insert(*tun, SocketAddr::new(IpAddr::from(*ethernet), PORT))
            .is_none());
    }
    map
});
