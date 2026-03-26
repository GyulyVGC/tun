#![allow(clippy::used_underscore_binding)]

use std::collections::HashMap;
use std::ops::Sub;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{panic, process};

use crate::cli::Args;
use crate::commands::{RtNetLinkHandle, cleanup_network, setup_br0};
use crate::control_channel::control_channel;
use crate::env::{CONTROL_SERVICE_ADDR, CONTROL_SERVICE_PORT};
use crate::forward::receive::receive;
use crate::forward::send::send;
use crate::local_endpoints::LocalEndpoints;
use crate::peers::peer::Peers;
use clap::Parser;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use nullnet_firewall::{DataLink, Firewall, FirewallError, LogLevel};
use nullnet_grpc_lib::NullnetGrpcInterface;
use nullnet_grpc_lib::nullnet_grpc::{Net, Services};
use nullnet_liberror::{Error, ErrorHandler, Location, location};
use tokio::sync::RwLock;
use tun_rs::{DeviceBuilder, Layer};

mod cli;
mod commands;
mod control_channel;
mod craft;
mod env;
mod forward;
mod local_endpoints;
mod peers;

pub const FORWARD_PORT: u16 = 9999;
pub const TAP_NAME: &str = "nullnet0";

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
        firewall_path,
        num_tasks,
        ..
    } = Args::parse();

    // create a handle to execute netlink commands
    let rtnetlink_handle = RtNetLinkHandle::new()?;

    // cleanup existing VLANs and VXLANs material
    cleanup_network(&rtnetlink_handle).await;

    // maps of all the peers
    let peers = Arc::new(RwLock::new(Peers::default()));
    let peers_2 = peers.clone();

    // create firewall based on the defined rules
    let mut firewall = Firewall::new();
    firewall.log_level(LogLevel::Db);
    firewall.data_link(DataLink::Ethernet);
    let firewall_shared = Arc::new(RwLock::new(firewall));
    set_firewall_rules(&firewall_shared, &firewall_path, true).await?;

    // initialize gRPC connection
    let grpc_server = grpc_init().await?;
    let grpc_server2 = grpc_server.clone();

    let net_type = grpc_server.network_type().await.handle_err(location!())?;

    if net_type.net() == Net::Vlan {
        setup_tap(num_tasks, peers, &firewall_shared, &rtnetlink_handle).await?;
        setup_br0(&rtnetlink_handle).await;
    }

    print_info(net_type.net());

    // read our services list from file and send it to the gRPC server
    tokio::spawn(async move {
        declare_services(grpc_server)
            .await
            .expect("Failed to declare services");
    });

    // listen on the gRPC control channel
    tokio::spawn(async move {
        control_channel(grpc_server2, peers_2, rtnetlink_handle)
            .await
            .expect("Control channel failed");
    });

    // watch the file defining rules and update the firewall accordingly
    set_firewall_rules(&firewall_shared, &firewall_path, false).await?;

    Ok(())
}

/// Prints useful info about the local environment and the created interface.
fn print_info(net: Net) {
    println!("\n{}", "=".repeat(40));
    println!("Nullnet is up and running!");
    println!("Network type: {net:?}");
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

async fn grpc_init() -> Result<NullnetGrpcInterface, Error> {
    let host = CONTROL_SERVICE_ADDR.to_string();
    let port = *CONTROL_SERVICE_PORT;

    let server = NullnetGrpcInterface::new(&host, port, false)
        .await
        .handle_err(location!())?;

    Ok(server)
}

async fn declare_services(grpc_server: NullnetGrpcInterface) -> Result<(), Error> {
    loop {
        // read services from file
        let services_toml = tokio::fs::read_to_string("services.toml")
            .await
            .handle_err(location!())?;
        let mut services: Services = toml::from_str(&services_toml).handle_err(location!())?;

        // get the map of logical name -> real container name (supports both standalone and Swarm)
        let running_containers = get_running_docker_containers().await;
        // get the list of actively listening ports on the host
        let listeners = listeners::get_all().handle_err(location!())?;

        // only declare services that are actually running
        // For Swarm, a single service name may map to multiple containers (replicas),
        // so we expand each service entry into one entry per running container.
        let file_services = services.services;
        services.services = Vec::new();
        for service in file_services {
            if let Some(container) = &service.docker_container {
                if let Some(real_names) = running_containers.get(container.as_str()) {
                    for real_name in real_names {
                        let mut s = service.clone();
                        s.docker_container = Some(real_name.clone());
                        services.services.push(s);
                    }
                }
            } else {
                // Host services: only declare if the port is actively listening
                if listeners
                    .iter()
                    .any(|listener| u32::from(listener.socket.port()) == service.port)
                {
                    services.services.push(service);
                }
            }
        }

        println!("Declaring services to gRPC server: {services:?}");

        // send services to gRPC server
        grpc_server
            .services_list(services)
            .await
            .handle_err(location!())?;

        // wait before re-declaring services
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

/// Returns a map of logical name -> real container names for all running Docker containers.
///
/// Supports both standalone Docker (name -> [name]) and Swarm mode (swarm service label -> [replicas]).
async fn get_running_docker_containers() -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();

    // Query container name and Swarm service label together
    let output = tokio::process::Command::new("docker")
        .args([
            "ps",
            "--format",
            "{{.Names}}\t{{.Label \"com.docker.swarm.service.name\"}}",
        ])
        .output()
        .await;

    if let Ok(out) = output {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            let real_name = parts[0].to_string();
            let swarm_label = parts.get(1).unwrap_or(&"").trim();
            if swarm_label.is_empty() {
                // standalone: logical name = container name
                map.entry(real_name.clone()).or_default().push(real_name);
            } else {
                // Swarm: logical name = swarm service label, may have multiple replicas
                map.entry(swarm_label.to_string())
                    .or_default()
                    .push(real_name);
            }
        }
    }

    map
}

async fn setup_tap(
    num_tasks: u8,
    peers: Arc<RwLock<Peers>>,
    firewall_shared: &Arc<RwLock<Firewall>>,
    rtnetlink_handle: &RtNetLinkHandle,
) -> Result<(), Error> {
    // set up the local environment
    let endpoints = LocalEndpoints::setup(rtnetlink_handle).await?;
    let forward_socket = endpoints.forward_socket.clone();

    // create the asynchronous TAP device, and split it into reader & writer halves
    let device = DeviceBuilder::new()
        .name(TAP_NAME)
        .layer(Layer::L2)
        // TODO: MTU? GSO?
        // .mtu(mtu)
        .build_async()
        .handle_err(location!())?;

    let reader_shared = Arc::new(device);
    let writer_shared = reader_shared.clone();

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
            Box::pin(receive(&writer, &socket_1, &firewall_1)).await;
        });

        // handle outgoing traffic
        tokio::spawn(async move {
            Box::pin(send(&reader, &socket_2, &firewall_2, peers_2)).await;
        });
    }

    Ok(())
}
