#![allow(clippy::used_underscore_binding)]

use local_ip_address::local_ip;
use std::net::{IpAddr, SocketAddr};
use std::ops::Sub;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{panic, process};

use clap::Parser;
use notify::event::ModifyKind;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use nullnet_firewall::{DataLink, Firewall, FirewallError};
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, RwLock};
use tun::{Configuration, Device};

use crate::cli::Args;
use crate::forward::receive::receive;
use crate::forward::send::send;
use crate::peers_discovery::peers_discovery;

mod cli;
mod craft;
mod forward;
mod frames;
mod peers;
mod peers_discovery;

const PORT: u16 = 9999;

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

    let (local_eth_ip, socket) = try_bind_socket_until_success().await;
    let socket_shared = Arc::new(socket);

    let tun_ip = get_tun_ip(&local_eth_ip);

    tokio::spawn(async move {
        peers_discovery(local_eth_ip).await;
    });

    let mut config = Configuration::default();
    set_tun_name(&tun_ip, &mut config);
    config
        .mtu(i32::try_from(mtu).unwrap())
        .address(tun_ip)
        .netmask((255, 255, 255, 0))
        .up();

    let device = tun::create_as_async(&config).expect("Failed to create TUN device");
    let device_name = device.get_ref().name().unwrap();
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
        let socket_1 = socket_shared.clone();
        let socket_2 = socket_shared.clone();
        let firewall_1 = firewall_shared.clone();
        let firewall_2 = firewall_shared.clone();

        tokio::spawn(async move {
            Box::pin(receive(&writer, &socket_1, &firewall_1, &tun_ip)).await;
        });

        tokio::spawn(async move {
            Box::pin(send(&reader, &socket_2, &firewall_2)).await;
        });
    }

    print_info(&local_eth_ip, &device_name, &tun_ip, mtu);

    set_firewall_rules(&firewall_shared, &firewall_path, false).await;
}

/// Tries to bind a UDP socket until success, retrying every 10 seconds.
async fn try_bind_socket_until_success() -> (IpAddr, UdpSocket) {
    loop {
        if let Ok(address) = local_ip() {
            println!("Local IP address found: {address}");
            let socket_addr = SocketAddr::new(address, PORT);
            if let Ok(socket) = UdpSocket::bind(socket_addr).await {
                return (address, socket);
            }
        }
        println!("Could not correctly bind a socket; will retry in 10 seconds...");
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

/// Returns an IP address for the TUN device (based on the local Ethernet IP and supposing /24 netmask).
fn get_tun_ip(local_eth_ip: &IpAddr) -> IpAddr {
    let local_eth_ip_string = local_eth_ip.to_string();
    let host_part = local_eth_ip_string.split('.').last().unwrap();
    let tun_ip_string = ["10.0.0.", host_part].concat();
    IpAddr::from_str(&tun_ip_string).unwrap()
}

/// Sets a name in the form 'nullnetX' for the TUN, where X is the host part of the TUN's ip (doesn't work on macOS).
///
/// Example: the TUN with address 10.0.0.1 will be called nullnet1 (this supposes netmask /24).
fn set_tun_name(_tun_ip: &IpAddr, _config: &mut Configuration) {
    #[cfg(not(target_os = "macos"))]
    _config.name(format!(
        "nullnet{}",
        _tun_ip.to_string().split('.').last().unwrap()
    ));
}

/// Manually setup routing on macOS (to be done after TUN creation).
fn configure_routing(_tun_ip: &IpAddr) {
    #[cfg(target_os = "macos")]
    process::Command::new("route")
        .args(["-n", "add", "-net", "10.0.0.0/24", &_tun_ip.to_string()])
        .spawn()
        .expect("Failed to configure routing");
}

/// Prints useful info about the created device.
fn print_info(local_eth_ip: &IpAddr, device_name: &str, device_addr: &IpAddr, mtu: usize) {
    println!("\n{}", "=".repeat(40));
    println!("UDP socket bound successfully:");
    println!("\t- address: {local_eth_ip}:{PORT}\n");
    println!("TUN device created successfully:");
    println!("\t- address: {device_addr}");
    println!("\t- name:    {device_name}");
    println!("\t- MTU:     {mtu} B");
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
