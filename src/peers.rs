use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::net::IpAddr;

static ETHERNET_TUN_TUPLES: Lazy<Vec<(IpAddr, IpAddr)>> = Lazy::new(|| {
    vec![
        // Proxmox VM 999
        (
            IpAddr::from([192, 168, 1, 162]),
            IpAddr::from([10, 0, 0, 1]),
        ),
        // Proxmox VM 997
        (
            IpAddr::from([192, 168, 1, 144]),
            IpAddr::from([10, 0, 0, 2]),
        ),
        // macOS
        (
            IpAddr::from([192, 168, 1, 113]),
            IpAddr::from([10, 0, 0, 3]),
        ),
    ]
});

pub static ETHERNET_TO_TUN: Lazy<HashMap<IpAddr, IpAddr>> = Lazy::new(|| {
    let mut map = HashMap::new();
    for (ethernet, tun) in ETHERNET_TUN_TUPLES.iter() {
        assert!(map.insert(ethernet.to_owned(), tun.to_owned()).is_none());
    }
    map
});

// pub static TUN_TO_ETHERNET: Lazy<HashMap<IpAddr, IpAddr>> = Lazy::new(|| {
//     let mut map = HashMap::new();
//     for (ethernet, tun) in ETHERNET_TUN_TUPLES.iter() {
//         assert!(map.insert(tun.to_owned(), ethernet.to_owned()).is_none());
//     }
//     map
// });
