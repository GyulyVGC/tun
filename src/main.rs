#![allow(clippy::used_underscore_binding)]

mod checksums;
mod cli;
mod os_frame;
mod peers;
mod receive;
mod reject_payloads;
mod send;
mod socket_frame;

use crate::cli::Args;
use crate::peers::SOCKET_TO_TUN;
use crate::receive::receive;
use crate::send::send;
use clap::Parser;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use nullnet_firewall::{DataLink, Firewall};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::{panic, process};
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tun::{Configuration, Device};

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
    } = Args::parse();

    let (src_socket, socket) = bind_socket(source).await;

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
    let (device_out, device_in) = tokio::io::split(device);

    configure_routing(tun_ip);

    let mut firewall = Firewall::new(&firewall_path).expect("Invalid firewall specification");
    firewall.data_link(DataLink::RawIP);
    firewall.log(log);
    let firewall_r1 = Arc::new(RwLock::new(firewall));
    let firewall_r2 = firewall_r1.clone();
    let firewall_w = firewall_r1.clone();

    tokio::spawn(async move {
        Box::pin(receive(device_in, &socket_in, &firewall_r1, tun_ip)).await;
    });

    tokio::spawn(async move {
        Box::pin(send(device_out, &socket_out, &firewall_r2)).await;
    });

    print_info(&src_socket, &device_name, tun_ip, mtu);

    update_firewall_on_press(&firewall_w, &firewall_path).await;
}

/// Tries to bind a UDP socket.
///
/// If `source` is `None`, this function will iterate over all the known peers until a valid socket can be opened.
async fn bind_socket(source: Option<IpAddr>) -> (SocketAddr, UdpSocket) {
    if let Some(address) = source {
        let socket_addr = SocketAddr::new(address, PORT);
        (
            socket_addr,
            UdpSocket::bind(socket_addr)
                .await
                .expect("Failed to bind socket"),
        )
    } else {
        for socket_addr in SOCKET_TO_TUN.keys() {
            if let Ok(socket) = UdpSocket::bind(socket_addr).await {
                return (*socket_addr, socket);
            }
        }
        panic!("None of the available IP addresses is in the list of known peers");
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
        .stdout(process::Stdio::null())
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

/// Allows to refresh the firewall rules definition when the `enter` key is pressed.
async fn update_firewall_on_press(firewall: &Arc<RwLock<Firewall>>, path: &str) {
    loop {
        if let Ok(Event::Key(KeyEvent {
            code,
            modifiers: _,
            kind,
            state: _,
        })) = crossterm::event::read()
        {
            if code.eq(&KeyCode::Enter) && kind.eq(&KeyEventKind::Press) {
                if let Err(err) = firewall.write().await.update_rules(path) {
                    println!("{err}");
                    println!("Firewall was not updated!");
                } else {
                    println!("Firewall has been updated!");
                }
            }
        }
    }
}
