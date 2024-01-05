use crate::os_frame::OsFrame;
use crate::peers::TUN_TO_SOCKET;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, ReadHalf};
use tokio::net::UdpSocket;
use tun::AsyncDevice;

pub async fn send(mut device: ReadHalf<AsyncDevice>, socket: Arc<UdpSocket>) {
    let mut os_frame = OsFrame::new();
    loop {
        // wait until there is a packet outgoing from kernel
        os_frame.actual_bytes = device.read(&mut os_frame.frame).await.unwrap_or(0);

        // send the packet to the socket
        let socket_buf = os_frame.to_socket_buf();
        let Some(dst_socket) = get_dst_socket(socket_buf) else {
            println!("sock sock");
            continue;
        };
        let x = socket.send_to(socket_buf, dst_socket).await.unwrap_or(0);
        if x == 0 {
            println!("mmh");
        }
    }
}

fn get_dst_socket(socket_buf: &[u8]) -> Option<&SocketAddr> {
    if socket_buf.len() < 20 {
        println!("tx hoy hoy");
        None
    } else {
        TUN_TO_SOCKET.get(&socket_buf[16..20])
    }
}
