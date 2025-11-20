// Open vSwitch x Nullnet
// we want to use TAPs over TUNs for layer 2 networking as done by Open vSwitch
// our main TAP should be a "trunk" port (i.e. an interface that carries traffic for multiple VLANs)
// each service should have its own TAP configured as an "access" port (i.e. an interface assigned to a single VLAN)
// this way, we can isolate traffic for different services while using a "central" TAP interface for overall connectivity
// this idea works under the assumption that incoming packets are already VLAN-tagged
// if this isn't the case, we need to use OVS rules to tag packets based on their IPs' subnet
// -------------------------------------------------------------------------------------------------
// $ ovs-vsctl add-br br0
// $ ovs-vsctl add-port br0 tap0
// $ ovs-vsctl add-port br0 vethx tag=x

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
use nullnet_liberror::{Error, ErrorHandler, Location, location};
use tokio::sync::{Mutex, RwLock};
use tun::{AbstractDevice, Configuration, Layer};

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

#[tokio::main]
async fn main() -> Result<(), Error> {
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
    let endpoints = LocalEndpoints::setup().await?;
    let tun_ip = endpoints.ips.tun;
    let netmask = endpoints.ips.netmask;
    let forward_socket = endpoints.sockets.forward.clone();

    // maps of all the peers
    let peers = Arc::new(RwLock::new(HashMap::new()));
    let peers_2 = peers.clone();

    // configure TUN device
    let mut config = Configuration::default();
    set_tun_name(&tun_ip, &netmask, &mut config);
    config
        .layer(Layer::L2)
        .mtu(mtu)
        .address(tun_ip)
        .netmask(netmask)
        .up();

    // create the asynchronous TUN device, and split it into reader & writer halves
    let device = tun::create_as_async(&config).handle_err(location!())?;
    let tun_name = device.tun_name().unwrap_or_default();
    let (read_half, write_half) = tokio::io::split(device);
    let reader_shared = Arc::new(Mutex::new(read_half));
    let writer_shared = Arc::new(Mutex::new(write_half));

    // properly setup routing tables
    // configure_routing(&tun_ip, &netmask);

    // create firewall based on the defined rules
    let mut firewall = Firewall::new();
    firewall.data_link(DataLink::Ethernet);
    let firewall_shared = Arc::new(RwLock::new(firewall));
    set_firewall_rules(&firewall_shared, &firewall_path, true).await?;

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
    set_firewall_rules(&firewall_shared, &firewall_path, false).await?;

    Ok(())
}

// /// Sets a name in the form 'nullnetX' for the TUN, where X is the host part of the TUN's ip (doesn't work on macOS).
// ///
// /// Example: the TUN with address 10.0.0.1 will be called nullnet1.
fn set_tun_name(_tun_ip: &IpAddr, _netmask: &IpAddr, _config: &mut Configuration) {
    // #[cfg(not(target_os = "macos"))]
    // {
    //     let tun_ip_octets = _tun_ip.into_address().octets();
    //     let netmask_octets = _netmask.into_address().octets();
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
    let Ok(forward_socket) = &local_endpoints.sockets.forward.local_addr() else {
        return;
    };
    let Ok(discovery_socket) = &local_endpoints.sockets.discovery.local_addr() else {
        return;
    };
    let Ok(discovery_broadcast_socket) = &local_endpoints.sockets.discovery_broadcast.local_addr()
    else {
        return;
    };
    println!("\n{}", "=".repeat(40));
    println!("UDP sockets bound successfully:");
    println!("    - forward:   {forward_socket}");
    println!("    - discovery: {discovery_socket}");
    println!("    - broadcast: {discovery_broadcast_socket}\n");
    println!("TUN device created successfully:");
    println!("    - address:   {tun_ip}");
    println!("    - netmask:   {netmask}");
    println!("    - name:      {tun_name}");
    println!("    - MTU:       {mtu} B");
    println!("{}\n", "=".repeat(40));
}

/// Loads and refreshes firewall rules whenever the corresponding file is updated.
async fn set_firewall_rules(
    firewall: &Arc<RwLock<Firewall>>,
    firewall_path: &str,
    is_init: bool,
) -> Result<(), Error> {
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
            return Ok(());
        }
    }

    let mut firewall_directory = PathBuf::from(firewall_path);
    firewall_directory.pop();

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = RecommendedWatcher::new(tx, Config::default()).handle_err(location!())?;
    watcher
        .watch(&firewall_directory, RecursiveMode::Recursive)
        .handle_err(location!())?;

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
                    return Ok(());
                }
                last_update_time = Instant::now();
            }
        }
    }
}
