use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::UdpSocket;
use std::{env, process};

const PORT: u16 = 9999;

pub static ETHERNET_TO_TUN_ADDRESSES: Lazy<HashMap<String, String>> = Lazy::new(|| {
    HashMap::from([
        (String::from("192.168.1.162"), String::from("10.0.0.1")),
        (String::from("192.168.1.144"), String::from("10.0.0.2")),
    ])
});

fn main() {
    let mut args = env::args().skip(1);
    let src_eth_address = match args.next() {
        Some(arg) => arg,
        None => {
            eprintln!("Expected CLI arguments: <src_eth_address> <dst_eth_address>");
            process::exit(1);
        }
    };
    let dst_eth_address = match args.next() {
        Some(arg) => arg,
        None => {
            eprintln!("Expected CLI arguments: <src_eth_address> <dst_eth_address>");
            process::exit(1);
        }
    };
    let src_socket_address = format!("{}:{}", src_eth_address, PORT);
    let dst_socket_address = format!("{}:{}", dst_eth_address, PORT);

    let mut config = tun::Configuration::default();
    config
        .address(ETHERNET_TO_TUN_ADDRESSES.get(&src_eth_address).unwrap())
        .netmask((255, 255, 255, 0))
        .up();

    // #[cfg(target_os = "linux")]
    // config.platform(|config| {
    //     config.packet_information(true);
    // });

    let mut dev = tun::create(&config).unwrap();
    let mut buf_out = [0; 4096];
    let mut buf_in = [0; 4096];

    let socket = UdpSocket::bind(src_socket_address).unwrap();
    // socket.set_nonblocking(true).unwrap();
    socket.connect(dst_socket_address).unwrap();

    loop {
        // read a packet from the kernel
        let num_bytes_out = dev.read(&mut buf_out).unwrap_or(0);
        // send the packet to the socket
        if num_bytes_out > 0 {
            socket.send(&buf_out[0..num_bytes_out]).unwrap_or(0);
            println!("OUT {:?}\n", &buf_out[0..num_bytes_out]);
        }

        // receive possible packet from the socket
        let num_bytes_in = socket.recv(&mut buf_in).unwrap_or(0);
        // write packet to the kernel
        if num_bytes_in > 0 {
            dev.write(&buf_in[0..num_bytes_in]).unwrap_or(0);
            println!("IN {:?}\n", &buf_in[0..num_bytes_in]);
        }
    }
}
