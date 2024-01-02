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
use tun::Configuration;

const PORT: u16 = 9999;

#[tokio::main]
async fn main() {
    let mut args = env::args().skip(1);
    let Some(src_eth_string) = args.next() else {
        eprintln!("Expected CLI arguments: <src_eth_address> <dst_eth_address>");
        process::exit(1);
    };
    let Some(dst_eth_string) = args.next() else {
        eprintln!("Expected CLI arguments: <src_eth_address> <dst_eth_address>");
        process::exit(1);
    };
    let src_eth_address =
        IpAddr::from_str(&src_eth_string).expect("CLI argument is not a valid IP");
    let dst_eth_address =
        IpAddr::from_str(&dst_eth_string).expect("CLI argument is not a valid IP");
    let src_socket_address = SocketAddr::new(src_eth_address, PORT);
    let dst_socket_address = SocketAddr::new(dst_eth_address, PORT);

    let mut config = tun::Configuration::default();
    set_tun_name(&src_eth_address, &mut config);
    config
        .address(
            ETHERNET_TO_TUN
                .get(&src_eth_address)
                .expect("Address is not in the list of peers"),
        )
        .netmask((255, 255, 255, 0))
        .up();

    let (device_out, device_in) = tun::create(&config)
        .expect("Failed to create TUN device")
        .split();

    configure_routing(&src_eth_address);

    let socket = UdpSocket::bind(src_socket_address)
        .await
        .expect("Failed to bind socket");
    let socket_in = Arc::new(socket);
    let socket_out = socket_in.clone();

    tokio::spawn(async move {
        receive(device_in, socket_in).await;
    });

    send(device_out, socket_out, dst_socket_address).await;
}

/// Returns a name in the form 'nullnetX' where X is the host part of the TUN's ip (doesn't work on macOS)
/// Example: the TUN with address 10.0.0.1 will be called nullnet1 (this supposes netmask /24)
fn set_tun_name(_src_eth_address: &IpAddr, _config: &mut Configuration) {
    #[cfg(not(target_os = "macos"))]
    {
        let tun_ip = ETHERNET_TO_TUN
            .get(_src_eth_address)
            .expect("Address is not in the list of peers")
            .to_string();
        let num = tun_ip.split('.').last().unwrap();
        _config.name(format!("nullnet{num}"));
    }
}

/// To work on macOS, the route must be setup manually (after TUN creation!)
fn configure_routing(_src_eth_address: &IpAddr) {
    #[cfg(target_os = "macos")]
    std::process::Command::new("route")
        .args([
            "-n",
            "add",
            "-net",
            "10.0.0.0/24",
            &ETHERNET_TO_TUN
                .get(_src_eth_address)
                .expect("Address is not in the list of peers")
                .to_string(),
        ])
        .spawn()
        .expect("Failed to configure routing");
}
