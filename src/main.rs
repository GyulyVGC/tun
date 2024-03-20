#![allow(clippy::used_underscore_binding)]

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::ops::Sub;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{panic, process};

use clap::Parser;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use nullnet_firewall::{DataLink, Firewall, FirewallError};
use tokio::sync::{Mutex, RwLock};
use tun2::{AbstractDevice, Configuration};

use crate::cli::Args;
use crate::forward::receive::receive;
use crate::forward::send::send;
use crate::local_endpoints::LocalEndpoints;
use crate::peers::discovery::discover_peers;

mod cli;
mod craft;
mod forward;
mod local_endpoints;
mod peers;

pub const FORWARD_PORT: u16 = 9999;
pub const DISCOVERY_PORT: u16 = FORWARD_PORT - 1;
pub const NETWORK: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 0));
pub const MULTICAST: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 255));

#[tokio::main]
async fn main() {
    // kill the main thread as soon as a secondary thread panics
    let orig_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // invoke the default handler and exit the process
        orig_hook(panic_info);
        process::exit(1);
    }));

    // read CLI arguments
    let Args {
        mtu,
        firewall_path,
        // log_path, TODO!
        // peers_path,
        num_tasks,
    } = Args::parse();

    // set up the local environment
    let endpoints = LocalEndpoints::setup().await;
    let tun_ip = endpoints.ips.tun;
    let netmask = endpoints.ips.netmask;
    let forward_socket = endpoints.sockets.forward.clone();

    // maps of all the peers
    let peers = Arc::new(RwLock::new(HashMap::new()));
    let peers_2 = peers.clone();

    // configure TUN device
    let mut config = Configuration::default();
    set_tun_name(&tun_ip, &netmask, &mut config);
    config.mtu(mtu).address(tun_ip).netmask(netmask).up();

    // create the asynchronous TUN device, and split it into reader & writer halves
    let device = tun2::create_as_async(&config).expect("Failed to create TUN device");
    let tun_name = device.as_ref().tun_name().unwrap_or_default();
    let (read_half, write_half) = tokio::io::split(device);
    let reader_shared = Arc::new(Mutex::new(read_half));
    let writer_shared = Arc::new(Mutex::new(write_half));

    // properly setup routing tables
    // configure_routing(&tun_ip, &netmask);

    // create firewall based on the defined rules
    let mut firewall = Firewall::new();
    firewall.data_link(DataLink::RawIP);
    let firewall_shared = Arc::new(RwLock::new(firewall));
    set_firewall_rules(&firewall_shared, &firewall_path, true).await;

    // spawn a number of asynchronous tasks to handle incoming and outgoing network traffic
    for _ in 0..num_tasks / 2 {
        let writer = writer_shared.clone();
        let reader = reader_shared.clone();
        let socket_1 = forward_socket.clone();
        let socket_2 = socket_1.clone();
        let firewall_1 = firewall_shared.clone();
        let firewall_2 = firewall_shared.clone();
        let peers_2 = peers.clone();

        // handle incoming traffic
        tokio::spawn(async move {
            Box::pin(receive(&writer, &socket_1, &firewall_1, &tun_ip)).await;
        });

        // handle outgoing traffic
        tokio::spawn(async move {
            Box::pin(send(&reader, &socket_2, &firewall_2, peers_2)).await;
        });
    }

    // print information about the overall setup
    print_info(&endpoints, &tun_name, mtu);

    // discover peers in the same area network
    tokio::spawn(async move {
        discover_peers(endpoints, peers_2).await;
    });

    // watch the file defining rules and update the firewall accordingly
    set_firewall_rules(&firewall_shared, &firewall_path, false).await;
}

// /// Sets a name in the form 'nullnetX' for the TUN, where X is the host part of the TUN's ip (doesn't work on macOS).
// ///
// /// Example: the TUN with address 10.0.0.1 will be called nullnet1.
fn set_tun_name(_tun_ip: &IpAddr, _netmask: &IpAddr, _config: &mut Configuration) {
    // #[cfg(not(target_os = "macos"))]
    // {
    //     let tun_ip_octets = _tun_ip.into_address().unwrap().octets();
    //     let netmask_octets = _netmask.into_address().unwrap().octets();
    //
    //     let mut host_octets = [0; 4];
    //     for i in 0..4 {
    //         host_octets[i] = tun_ip_octets[i] & !netmask_octets[i];
    //     }
    //
    //     let host_num = u32::from_be_bytes(host_octets);
    //     _config.name(format!("nullnet{host_num}"));
    // }
}

// /// Manually setup routing on macOS (to be done after TUN creation).
// fn configure_routing(_tun_ip: &IpAddr, _netmask: &IpAddr) {
//     #[cfg(target_os = "macos")]
//     {
//         process::Command::new("route")
//             .args([
//                 "-n",
//                 "add",
//                 "-net",
//                 &NETWORK.to_string(),
//                 &_tun_ip.to_string(),
//                 "-netmask",
//                 &_netmask.to_string(),
//             ])
//             .spawn()
//             .expect("Failed to configure routing");
//     }
// }

/// Prints useful info about the local environment and the created interface.
fn print_info(local_endpoints: &LocalEndpoints, tun_name: &str, mtu: u16) {
    let tun_ip = &local_endpoints.ips.tun;
    let netmask = &local_endpoints.ips.netmask;
    let forward_socket = &local_endpoints.sockets.forward.local_addr().unwrap();
    let discovery_socket = &local_endpoints.sockets.discovery.local_addr().unwrap();
    let discovery_multicast_socket = &local_endpoints
        .sockets
        .discovery_multicast
        .local_addr()
        .unwrap();
    println!("\n{}", "=".repeat(40));
    println!("UDP sockets bound successfully:");
    println!("    - forward:   {forward_socket}");
    println!("    - discovery: {discovery_socket}");
    println!("    - multicast: {discovery_multicast_socket}\n");
    println!("TUN device created successfully:");
    println!("    - address:   {tun_ip}");
    println!("    - netmask:   {netmask}");
    println!("    - name:      {tun_name}");
    println!("    - MTU:       {mtu} B");
    println!("{}\n", "=".repeat(40));
}

/// Loads and refreshes firewall rules whenever the corresponding file is updated.
async fn set_firewall_rules(firewall: &Arc<RwLock<Firewall>>, firewall_path: &str, is_init: bool) {
    let print_info = |result: &Result<(), FirewallError>, is_init: bool| match result {
        Err(err) => {
            println!("{err}");
            if is_init {
                println!("Waiting for a valid firewall file...");
            } else {
                println!("Firewall was not updated!");
            }
        }
        Ok(()) => {
            if is_init {
                println!("A valid firewall has been instantiated!");
            } else {
                println!("Firewall has been updated!");
            }
        }
    };

    if is_init {
        let result = firewall.write().await.set_rules(firewall_path);
        print_info(&result, is_init);
        if result.is_ok() {
            return;
        }
    }

    let mut firewall_directory = PathBuf::from(firewall_path);
    firewall_directory.pop();

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = RecommendedWatcher::new(tx, Config::default()).unwrap();
    watcher
        .watch(&firewall_directory, RecursiveMode::Recursive)
        .unwrap();

    let mut last_update_time = Instant::now().sub(Duration::from_secs(60));

    loop {
        // only update rules if the event is related to a file change
        if let Ok(Ok(Event {
            kind: EventKind::Modify(_),
            ..
        })) = rx.recv()
        {
            // debounce duplicated events
            if last_update_time.elapsed().as_millis() > 100 {
                // ensure file changes are propagated
                tokio::time::sleep(Duration::from_millis(100)).await;
                let result = firewall.write().await.set_rules(firewall_path);
                print_info(&result, is_init);
                if result.is_ok() && is_init {
                    return;
                }
                last_update_time = Instant::now();
            }
        }
    }
}
