use crate::os_frame::OsFrame;
use crate::peers::TUN_TO_SOCKET;
use nullnet_firewall::{Firewall, FirewallAction, FirewallDirection};
use std::io::Read;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::sync::{Arc, RwLock};
use tun::platform::posix::Reader;

pub fn send(
    mut device: Reader,
    socket: &Arc<UdpSocket>,
    firewall: &Arc<RwLock<Firewall>>,
    mtu: usize,
) {
    let mut os_frame = OsFrame::new(mtu);
    loop {
        // wait until there is a packet outgoing from kernel
        os_frame.actual_bytes = device.read(&mut os_frame.frame).unwrap_or(0);

        println!("ho");
        if os_frame.actual_bytes > 0 {
            println!("ho ho");
            // send the packet to the socket
            let socket_buf = os_frame.to_socket_buf();
            let Some(dst_socket) = get_dst_socket(socket_buf) else {
                continue;
            };
            match firewall
                .read()
                .unwrap()
                .resolve_packet(socket_buf, FirewallDirection::OUT)
            {
                FirewallAction::ACCEPT => socket.send_to(socket_buf, dst_socket).unwrap_or(0),
                FirewallAction::DENY | FirewallAction::REJECT => 0,
            };
        }
    }
}

fn get_dst_socket(socket_buf: &[u8]) -> Option<&SocketAddr> {
    if socket_buf.len() < 20 {
        None
    } else {
        TUN_TO_SOCKET.get(&socket_buf[16..20])
    }
}
