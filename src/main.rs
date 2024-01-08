#![allow(clippy::used_underscore_binding)]

mod os_frame;
mod peers;
mod receive;
mod send;
mod socket_frame;

use crate::peers::ETHERNET_TO_TUN;
use crate::receive::receive;
use crate::send::send;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use nullnet_firewall::{DataLink, Firewall};
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::{env, panic, process, thread};
use tun::Configuration;

const PORT: u16 = 9999;
const MTU: usize = 1500 - 20 - 8;
const FIREWALL_PATH: &str = "./firewall.txt";

fn main() {
    // kill the main thread as soon as a secondary thread panics
    let orig_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // invoke the default handler and exit the process
        orig_hook(panic_info);
        process::exit(1);
    }));

    let src_socket_ip = parse_cli_args();

    let src_socket = SocketAddr::new(src_socket_ip, PORT);

    let mut config = Configuration::default();
    set_tun_name(&src_socket_ip, &mut config);
    config
        .mtu(i32::try_from(MTU).unwrap())
        .address(
            ETHERNET_TO_TUN
                .get(&src_socket_ip)
                .expect("Address is not in the list of peers"),
        )
        .netmask((255, 255, 255, 0))
        .up();

    let (device_out, device_in) = tun::create(&config)
        .expect("Failed to create TUN device")
        .split();

    configure_routing(&src_socket_ip);

    let socket = UdpSocket::bind(src_socket).expect("Failed to bind socket");
    let socket_in = Arc::new(socket);
    let socket_out = socket_in.clone();

    let mut firewall = Firewall::new(FIREWALL_PATH).expect("Invalid firewall specification");
    firewall.data_link(DataLink::RawIP);
    firewall.log(false);
    let firewall_r1 = Arc::new(RwLock::new(firewall));
    let firewall_r2 = firewall_r1.clone();
    let firewall_w = firewall_r1.clone();

    thread::spawn(move || {
        receive(device_in, &socket_in, &firewall_r1);
    });

    thread::spawn(move || {
        send(device_out, &socket_out, &firewall_r2);
    });

    update_firewall_on_press(&firewall_w);
}

fn parse_cli_args() -> IpAddr {
    let mut args = env::args().skip(1);

    let Some(src_socket_ip_string) = args.next() else {
        eprintln!("Expected CLI arguments: <src_socket_ip>");
        process::exit(1);
    };
    if args.next().is_some() {
        eprintln!("Expected CLI arguments: <src_socket_ip>");
        process::exit(1);
    }

    IpAddr::from_str(&src_socket_ip_string).expect("Invalid CLI argument: <src_socket_ip>")
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

fn update_firewall_on_press(firewall: &Arc<RwLock<Firewall>>) {
    loop {
        if let Ok(Event::Key(KeyEvent {
            code,
            modifiers: _,
            kind,
            state: _,
        })) = crossterm::event::read()
        {
            if code.eq(&KeyCode::Enter) && kind.eq(&KeyEventKind::Press) {
                firewall
                    .write()
                    .unwrap()
                    .update_rules(FIREWALL_PATH)
                    .unwrap();
            }
        }
    }
}
