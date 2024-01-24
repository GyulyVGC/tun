#![allow(clippy::used_underscore_binding)]

use std::net::{IpAddr, SocketAddr};
use std::ops::Sub;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{panic, process, thread};

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
use crate::peers::SOCKET_TO_TUN;

mod cli;
mod craft;
mod forward;
mod frames;
mod peers;

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
        source,
        log,
        mtu,
        firewall_path,
        num_tasks,
    } = Args::parse();

    let (src_socket, socket) = try_bind_socket_until_success(source).await;

    let socket_in = Arc::new(socket);
    let socket_out = socket_in.clone();

    let tun_ip = SOCKET_TO_TUN
        .get(&src_socket)
        .expect("Address is not in the list of known peers");

    let mut config = Configuration::default();
    set_tun_name(tun_ip, &mut config);
    config
        .mtu(i32::try_from(mtu).unwrap())
        .address(tun_ip)
        .netmask((255, 255, 255, 0))
        .up();

    let device = tun::create_as_async(&config).expect("Failed to create TUN device");
    let device_name = device.get_ref().name().unwrap();
    let (read_half, write_half) = tokio::io::split(device);
    let device_out = Arc::new(Mutex::new(read_half));
    let device_in = Arc::new(Mutex::new(write_half));

    configure_routing(tun_ip);

    let mut firewall = try_new_firewall_until_success(&firewall_path);
    firewall.data_link(DataLink::RawIP);
    firewall.log(log);
    let firewall_reader = Arc::new(RwLock::new(firewall));
    let firewall_writer = firewall_reader.clone();

    assert!(
        num_tasks >= 2,
        "The number of asynchronous tasks should be >= 2"
    );
    for _ in 0..num_tasks / 2 {
        let device_in_task = device_in.clone();
        let device_out_task = device_out.clone();
        let socket_in_task = socket_in.clone();
        let socket_out_task = socket_out.clone();
        let firewall_reader_task_1 = firewall_reader.clone();
        let firewall_reader_task_2 = firewall_reader.clone();

        tokio::spawn(async move {
            Box::pin(receive(
                &device_in_task,
                &socket_in_task,
                &firewall_reader_task_1,
                tun_ip,
            ))
            .await;
        });

        tokio::spawn(async move {
            Box::pin(send(
                &device_out_task,
                &socket_out_task,
                &firewall_reader_task_2,
            ))
            .await;
        });
    }

    print_info(&src_socket, &device_name, tun_ip, mtu);

    update_firewall_on_rules_change(&firewall_writer, &firewall_path).await;
}

/// Tries to bind a UDP socket.
///
/// This function will iterate over all the known peers until a valid socket can be opened.
async fn try_bind_socket_until_success(source: Option<IpAddr>) -> (SocketAddr, UdpSocket) {
    loop {
        if let Some(address) = source {
            let socket_addr = SocketAddr::new(address, PORT);
            if let Ok(socket) = UdpSocket::bind(socket_addr).await {
                return (socket_addr, socket);
            }
        } else {
            for socket_addr in SOCKET_TO_TUN.keys() {
                if let Ok(socket) = UdpSocket::bind(socket_addr).await {
                    return (*socket_addr, socket);
                }
            }
        }
        println!("None of the available IP addresses is in the list of known peers (will retry in 10 seconds...)");
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

/// Returns a name in the form 'nullnetX' where X is the host part of the TUN's ip (doesn't work on macOS)
///
/// Example: the TUN with address 10.0.0.1 will be called nullnet1 (this supposes netmask /24)
fn set_tun_name(_tun_ip: &IpAddr, _config: &mut Configuration) {
    #[cfg(not(target_os = "macos"))]
    _config.name(format!(
        "nullnet{}",
        _tun_ip.to_string().split('.').last().unwrap()
    ));
}

/// To work on macOS, the route must be setup manually (after TUN creation!)
fn configure_routing(_tun_ip: &IpAddr) {
    #[cfg(target_os = "macos")]
    process::Command::new("route")
        .args(["-n", "add", "-net", "10.0.0.0/24", &_tun_ip.to_string()])
        .spawn()
        .expect("Failed to configure routing");
}

fn print_info(src_socket: &SocketAddr, device_name: &str, device_addr: &IpAddr, mtu: usize) {
    println!("{}", "=".repeat(40));
    println!("UDP socket bound successfully:");
    println!("\t- address: {src_socket}");
    println!();
    println!("TUN device created successfully:");
    println!("\t- address: {device_addr}");
    println!("\t- name:    {device_name}");
    println!("\t- MTU:     {mtu} B");
    println!("{}", "=".repeat(40));
    println!();
}

/// Allows to refresh the firewall rules whenever the corresponding file is updated.
async fn update_firewall_on_rules_change(firewall: &Arc<RwLock<Firewall>>, firewall_path: &str) {
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
                if let Err(err) = firewall.write().await.update_rules(firewall_path) {
                    println!("{err}");
                    println!("Firewall was not updated!");
                } else {
                    println!("Firewall has been updated!");
                }
                last_update_time = Instant::now();
            }
        }
    }
}

/// Returns a new firewall from the rules in a file, waiting for valid rules in case of initial failure.
fn try_new_firewall_until_success(firewall_path: &str) -> Firewall {
    let print_new_firewall_info = |result: &Result<Firewall, FirewallError>| match result {
        Err(err) => {
            println!("{err}");
            println!("Waiting for a valid firewall file...");
        }
        Ok(_) => {
            println!("A valid firewall has been instantiated!");
        }
    };

    let result = Firewall::new(firewall_path);
    print_new_firewall_info(&result);
    if let Ok(firewall) = result {
        return firewall;
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
        // only try to instantiate a new firewall if the event is related to a file content change
        if let Ok(Ok(Event {
            kind: EventKind::Modify(ModifyKind::Data(_)),
            ..
        })) = rx.recv()
        {
            // debounce duplicated events
            if last_update_time.elapsed().as_millis() > 100 {
                // ensure file changes are propagated
                thread::sleep(Duration::from_millis(100));
                let result = Firewall::new(firewall_path);
                print_new_firewall_info(&result);
                if let Ok(firewall) = result {
                    return firewall;
                }
                last_update_time = Instant::now();
            }
        }
    }
}
