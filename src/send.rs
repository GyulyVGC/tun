use crate::os_frame::OsFrame;
use crate::peers::TUN_TO_SOCKET;
use nullnet_firewall::{Firewall, FirewallAction, FirewallDirection};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, ReadHalf};
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, RwLock};
use tun::AsyncDevice;

pub async fn send(
    device: &Arc<Mutex<ReadHalf<AsyncDevice>>>,
    socket: &Arc<UdpSocket>,
    firewall: &Arc<RwLock<Firewall>>,
) {
    let mut os_frame = OsFrame::new();
    loop {
        // wait until there is a packet outgoing from kernel
        os_frame.actual_bytes = device
            .lock()
            .await
            .read(&mut os_frame.frame)
            .await
            .unwrap_or(0);

        // send the packet to the socket
        let socket_buf = os_frame.to_socket_buf();
        let Some(dst_socket) = get_dst_socket(socket_buf) else {
            continue;
        };
        match firewall
            .read()
            .await
            .resolve_packet(socket_buf, FirewallDirection::OUT)
        {
            FirewallAction::ACCEPT => {
                socket.send_to(socket_buf, dst_socket).await.unwrap_or(0);
            }
            FirewallAction::DENY | FirewallAction::REJECT => {}
        };
    }
}

fn get_dst_socket(socket_buf: &[u8]) -> Option<&SocketAddr> {
    if socket_buf.len() < 20 {
        None
    } else {
        TUN_TO_SOCKET.get(&socket_buf[16..20])
    }
}
