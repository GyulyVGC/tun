#![allow(clippy::used_underscore_binding)]

mod os_frame;
mod peers;
mod receive;
mod send;
mod socket_frame;

use crate::peers::ETHERNET_TO_TUN;
use crate::receive::receive;
use crate::send::send;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::{env, process};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tun::Configuration;

const PORT: u16 = 9999;

const MTU: usize = 1500;

#[tokio::main]
async fn main() {
    ///////////////////////////////////////////////////////
    ctrlc::set_handler(move || {
        process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");
    ///////////////////////////////////////////////////////

    let (src_socket_ip, num_tasks) = parse_cli_args();

    let src_socket = SocketAddr::new(src_socket_ip, PORT);

    let mut config = Configuration::default();
    set_tun_name(&src_socket_ip, &mut config);
    config
        // .queues() ?
        .mtu(i32::try_from(MTU).unwrap())
        .address(
            ETHERNET_TO_TUN
                .get(&src_socket_ip)
                .expect("Address is not in the list of peers"),
        )
        .netmask((255, 255, 255, 0))
        .up();

    let (read_half, write_half) =
        tokio::io::split(tun::create_as_async(&config).expect("Failed to create TUN device"));
    let device_out = Arc::new(Mutex::new(read_half));
    let device_in = Arc::new(Mutex::new(write_half));

    configure_routing(&src_socket_ip);

    let socket = UdpSocket::bind(src_socket)
        .await
        .expect("Failed to bind socket");
    let socket_in = Arc::new(socket);
    let socket_out = socket_in.clone();

    for i in 0..num_tasks / 2 - 1 {
        let device_in_task = device_in.clone();
        let device_out_task = device_out.clone();
        let socket_in_task = socket_in.clone();
        let socket_out_task = socket_out.clone();

        tokio::spawn(async move {
            receive(device_in_task, socket_in_task, i).await;
        });

        tokio::spawn(async move {
            send(device_out_task, socket_out_task, i).await;
        });
    }
    tokio::spawn(async move {
        send(device_out, socket_out, num_tasks / 2).await;
    });
    receive(device_in, socket_in, num_tasks / 2).await;
}

fn parse_cli_args() -> (IpAddr, usize) {
    let mut args = env::args().skip(1);

    let Some(src_socket_ip_string) = args.next() else {
        eprintln!("Expected CLI arguments: <src_socket_ip> <num_tasks>");
        process::exit(1);
    };
    let Some(num_tasks_string) = args.next() else {
        eprintln!("Expected CLI arguments: <src_socket_ip> <num_tasks>");
        process::exit(1);
    };
    if args.next().is_some() {
        eprintln!("Expected CLI arguments: <src_socket_ip> <num_tasks>");
        process::exit(1);
    }

    (
        IpAddr::from_str(&src_socket_ip_string).expect("Invalid CLI argument: <src_socket_ip>"),
        usize::from_str(&num_tasks_string).expect("Invalid CLI argument: <num_tasks>"),
    )
}

/// Returns a name in the form 'nullnetX' where X is the host part of the TUN's ip (doesn't work on macOS)
/// Example: the TUN with address 10.0.0.1 will be called nullnet1 (this supposes netmask /24)
fn set_tun_name(_src_socket_ip: &IpAddr, _config: &mut Configuration) {
    #[cfg(not(target_os = "macos"))]
    {
        let tun_ip = ETHERNET_TO_TUN
            .get(_src_socket_ip)
            .expect("Address is not in the list of peers")
            .to_string();
        let num = tun_ip.split('.').last().unwrap();
        _config.name(format!("nullnet{num}"));
    }
}

/// To work on macOS, the route must be setup manually (after TUN creation!)
fn configure_routing(_src_socket_ip: &IpAddr) {
    #[cfg(target_os = "macos")]
    process::Command::new("route")
        .args([
            "-n",
            "add",
            "-net",
            "10.0.0.0/24",
            &ETHERNET_TO_TUN
                .get(_src_socket_ip)
                .expect("Address is not in the list of peers")
                .to_string(),
        ])
        .spawn()
        .expect("Failed to configure routing");
}
