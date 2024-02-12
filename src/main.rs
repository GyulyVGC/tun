#![allow(clippy::used_underscore_binding)]

use std::collections::HashMap;
use std::net::IpAddr;
use std::ops::Sub;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{panic, process};

use clap::Parser;
use notify::event::ModifyKind;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use nullnet_firewall::{DataLink, Firewall, FirewallError};
use tokio::sync::{Mutex, RwLock};
use tun::{Configuration, Device};

use crate::cli::Args;
use crate::forward::receive::receive;
use crate::forward::send::send;
use crate::local_endpoints::LocalEndpoints;
use crate::peers::discovery::discover_peers;

mod cli;
mod craft;
mod forward;
mod frames;
mod local_endpoints;
mod peers;

#[tokio::main]
async fn main() {
    // kill the main thread as soon as a secondary thread panics
    let orig_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // invoke the default handler and exit the process
        orig_hook(panic_info);
        process::exit(1);
    }));

    let Args {
        mtu,
        firewall_path,
        num_tasks,
    } = Args::parse();

    let endpoints = LocalEndpoints::new().await;
    let endpoints_2 = endpoints.clone();
    // tun ip to socket address map of all the discovered peers
    let peers = Arc::new(RwLock::new(HashMap::new()));
    let peers_2 = peers.clone();

    tokio::spawn(async move {
        discover_peers(endpoints_2, peers_2).await;
    });

    let tun_ip = endpoints.ips.tun;

    let mut config = Configuration::default();
    set_tun_name(&tun_ip, &endpoints.netmask, &mut config);
    config
        .mtu(i32::try_from(mtu).unwrap())
        .address(tun_ip)
        .netmask(endpoints.netmask)
        .up();

    let device = tun::create_as_async(&config).expect("Failed to create TUN device");
    let tun_name = device.get_ref().name().unwrap();
    let (read_half, write_half) = tokio::io::split(device);
    let reader_shared = Arc::new(Mutex::new(read_half));
    let writer_shared = Arc::new(Mutex::new(write_half));

    configure_routing(&tun_ip);

    let mut firewall = Firewall::new();
    firewall.data_link(DataLink::RawIP);
    let firewall_shared = Arc::new(RwLock::new(firewall));
    set_firewall_rules(&firewall_shared, &firewall_path, true).await;

    for _ in 0..num_tasks / 2 {
        let writer = writer_shared.clone();
        let reader = reader_shared.clone();
        let socket_1 = endpoints.sockets.forward.clone();
        let socket_2 = socket_1.clone();
        let firewall_1 = firewall_shared.clone();
        let firewall_2 = firewall_shared.clone();
        let peers_2 = peers.clone();

        tokio::spawn(async move {
            Box::pin(receive(&writer, &socket_1, &firewall_1, &tun_ip)).await;
        });

        tokio::spawn(async move {
            Box::pin(send(&reader, &socket_2, &firewall_2, peers_2)).await;
        });
    }

    print_info(&endpoints, &tun_name, mtu);

    set_firewall_rules(&firewall_shared, &firewall_path, false).await;
}

/// Sets a name in the form 'nullnetX' for the TUN, where X is the host part of the TUN's ip (doesn't work on macOS).
///
/// Example: the TUN with address 10.0.0.1 will be called nullnet1.
fn set_tun_name(_tun_ip: &IpAddr, _netmask: &IpAddr, _config: &mut Configuration) {
    #[cfg(not(target_os = "macos"))]
    {
        use tun::IntoAddress;
        let tun_ip_octets = _tun_ip.into_address().unwrap().octets();
        let netmask_octets = _netmask.into_address().unwrap().octets();

        let mut host_octets = [0; 4];
        for i in 0..4 {
            host_octets[i] = tun_ip_octets[i] & !netmask_octets[i];
        }

        let host_num = u32::from_be_bytes(host_octets);
        _config.name(format!("nullnet{host_num}"));
    }
}

/// Manually setup routing on macOS (to be done after TUN creation).
fn configure_routing(_tun_ip: &IpAddr) {
    #[cfg(target_os = "macos")]
    process::Command::new("route")
        // TODO: support every kind of netmask.
        .args(["-n", "add", "-net", "10.0.0.0/24", &_tun_ip.to_string()])
        .spawn()
        .expect("Failed to configure routing");
}

// this could be a Display impl of LocalEndpoints... TODO!
/// Prints useful info about the created device.
fn print_info(local_endpoints: &LocalEndpoints, tun_name: &str, mtu: usize) {
    let tun_ip = &local_endpoints.ips.tun;
    let netmask = &local_endpoints.netmask;
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

/// Refreshes the firewall rules whenever the corresponding file is updated.
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
        .watch(&firewall_directory, RecursiveMode::NonRecursive)
        .unwrap();

    let mut last_update_time = Instant::now().sub(Duration::from_secs(60));

    loop {
        // only update rules if the event is related to a file content change
        if let Ok(Ok(Event {
            kind: EventKind::Modify(ModifyKind::Data(_)),
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
