mod peers;
mod receive;
mod send;

use crate::peers::ETHERNET_TO_TUN;
use crate::receive::receive;
use crate::send::send;
use std::net::{IpAddr, SocketAddr};
use std::process::Command;
use std::str::FromStr;
use std::sync::Arc;
use std::{env, io, process};
use tokio::net::UdpSocket;

const PORT: u16 = 9999;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut args = env::args().skip(1);
    let Some(src_eth_string) = args.next() else {
        eprintln!("Expected CLI arguments: <src_eth_address> <dst_eth_address>");
        process::exit(1);
    };
    let Some(dst_eth_string) = args.next() else {
        eprintln!("Expected CLI arguments: <src_eth_address> <dst_eth_address>");
        process::exit(1);
    };
    let src_eth_address = IpAddr::from_str(&src_eth_string).unwrap();
    let dst_eth_address = IpAddr::from_str(&dst_eth_string).unwrap();
    let src_socket_address = SocketAddr::new(src_eth_address, PORT);
    let dst_socket_address = SocketAddr::new(dst_eth_address, PORT);

    let mut config = tun::Configuration::default();
    config
        .address(ETHERNET_TO_TUN.get(&src_eth_address).unwrap())
        .netmask((255, 255, 255, 0))
        .up();

    let (device_out, device_in) = tun::create(&config).unwrap().split();

    // to work on macOS, the route must be setup manually
    #[cfg(target_os = "macos")]
    Command::new("route")
        .args([
            "-n",
            "add",
            "-net",
            "10.0.0.0/24",
            &ETHERNET_TO_TUN.get(&src_eth_address).unwrap().to_string(),
        ])
        .spawn()
        .unwrap();

    let socket = UdpSocket::bind(src_socket_address).await?;
    let socket_in = Arc::new(socket);
    let socket_out = socket_in.clone();

    tokio::spawn(async move {
        receive(device_in, socket_in).await.unwrap();
    });

    send(device_out, socket_out, dst_socket_address).await?;

    Ok(())
}
