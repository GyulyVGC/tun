#![allow(clippy::used_underscore_binding)]

mod cli;
mod os_frame;
mod peers;
mod receive;
mod send;
mod socket_frame;

use crate::cli::Args;
use crate::peers::ETHERNET_TO_TUN;
use crate::receive::receive;
use crate::send::send;
use clap::Parser;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use nullnet_firewall::{DataLink, Firewall};
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::{Arc, RwLock};
use std::{panic, process, thread};
use tun::Configuration;

const PORT: u16 = 9999;

fn main() {
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

    let src_socket = SocketAddr::new(source, PORT);

    let mut config = Configuration::default();
    set_tun_name(&source, &mut config);
    config
        .mtu(i32::try_from(mtu).unwrap())
        .address(
            ETHERNET_TO_TUN
                .get(&source)
                .expect("Address is not in the list of peers"),
        )
        .netmask((255, 255, 255, 0))
        .up();

    let (device_out, device_in) = tun::create(&config)
        .expect("Failed to create TUN device")
        .split();

    configure_routing(&source);

    let socket = UdpSocket::bind(src_socket).expect("Failed to bind socket");
    let socket_in = Arc::new(socket);
    let socket_out = socket_in.clone();

    let mut firewall = Firewall::new(&firewall_path).expect("Invalid firewall specification");
    firewall.data_link(DataLink::RawIP);
    firewall.log(log);
    let firewall_r1 = Arc::new(RwLock::new(firewall));
    let firewall_r2 = firewall_r1.clone();
    let firewall_w = firewall_r1.clone();

    thread::spawn(move || {
        receive(device_in, &socket_in, &firewall_r1, mtu);
    });

    thread::spawn(move || {
        send(device_out, &socket_out, &firewall_r2, mtu);
    });

    update_firewall_on_press(&firewall_w, &firewall_path);
}

/// Returns a name in the form 'nullnetX' where X is the host part of the TUN's ip (doesn't work on macOS)
/// Example: the TUN with address 10.0.0.1 will be called nullnet1 (this supposes netmask /24)
fn set_tun_name(_source: &IpAddr, _config: &mut Configuration) {
    #[cfg(not(target_os = "macos"))]
    {
        let tun_ip = ETHERNET_TO_TUN
            .get(_source)
            .expect("Address is not in the list of peers")
            .to_string();
        let num = tun_ip.split('.').last().unwrap();
        _config.name(format!("nullnet{num}"));
    }
}

/// To work on macOS, the route must be setup manually (after TUN creation!)
fn configure_routing(_source: &IpAddr) {
    #[cfg(target_os = "macos")]
    process::Command::new("route")
        .args([
            "-n",
            "add",
            "-net",
            "10.0.0.0/24",
            &ETHERNET_TO_TUN
                .get(_source)
                .expect("Address is not in the list of peers")
                .to_string(),
        ])
        .spawn()
        .expect("Failed to configure routing");
}

fn update_firewall_on_press(firewall: &Arc<RwLock<Firewall>>, path: &str) {
    loop {
        if let Ok(Event::Key(KeyEvent {
            code,
            modifiers: _,
            kind,
            state: _,
        })) = crossterm::event::read()
        {
            if code.eq(&KeyCode::Enter) && kind.eq(&KeyEventKind::Press) {
                firewall.write().unwrap().update_rules(path).unwrap();
                println!("Firewall has been updated!");
            }
        }
    }
}
