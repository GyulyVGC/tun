use crate::os_frame::OsFrame;
use crate::peers::TUN_TO_ETHERNET;
use crate::PORT;
use std::io::Read;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tun::platform::posix::Reader;

pub async fn send(mut device: Reader, socket: Arc<UdpSocket>) {
    let mut os_frame = OsFrame::new();
    loop {
        // wait until there is a packet outgoing from kernel
        os_frame.actual_bytes = device.read(&mut os_frame.frame).unwrap_or(0);
        // send the packet to the socket
        if os_frame.actual_bytes > 0 {
            let socket_buf = os_frame.to_socket_buf();

            let Some(dst_tun_ip) = get_dst_tun_ip(socket_buf) else {
                continue;
            };
            println!("OUT to {dst_tun_ip}:\n{socket_buf:?}\n");

            let Some(dst_socket) = get_dst_socket(dst_tun_ip) else {
                continue;
            };

            socket.send_to(socket_buf, dst_socket).await.unwrap_or(0);
        }
    }
}

fn get_dst_tun_ip(socket_buf: &[u8]) -> Option<IpAddr> {
    if socket_buf.len() < 20 {
        None
    } else {
        let mut dst_tun_ip_octects = [0; 4];
        dst_tun_ip_octects.clone_from_slice(&socket_buf[16..20]);
        Some(IpAddr::from(dst_tun_ip_octects))
    }
}

fn get_dst_socket(dst_tun_ip: IpAddr) -> Option<SocketAddr> {
    let dst_socket_ip = TUN_TO_ETHERNET.get(&dst_tun_ip);
    dst_socket_ip.map(|address| SocketAddr::new(*address, PORT))
}
