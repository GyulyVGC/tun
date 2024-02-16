use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use nullnet_firewall::{Firewall, FirewallAction, FirewallDirection};
use tokio::io::{AsyncReadExt, ReadHalf};
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, RwLock};
use tun::AsyncDevice;

use crate::frames::os_frame::OsFrame;
use crate::peers::peer::{PeerKey, PeerVal};

pub async fn send(
    device: &Arc<Mutex<ReadHalf<AsyncDevice>>>,
    socket: &Arc<UdpSocket>,
    firewall: &Arc<RwLock<Firewall>>,
    peers: Arc<RwLock<HashMap<PeerKey, PeerVal>>>,
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

        if os_frame.actual_bytes > 0 {
            // send the packet to the socket
            let socket_buf = os_frame.to_socket_buf();
            let Some(dst_socket) = get_dst_socket(socket_buf, &peers).await else {
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
}

async fn get_dst_socket(
    socket_buf: &[u8],
    peers: &Arc<RwLock<HashMap<PeerKey, PeerVal>>>,
) -> Option<SocketAddr> {
    if socket_buf.len() < 20 {
        None
    } else {
        let dest_ip_slice: [u8; 4] = socket_buf[16..20].try_into().unwrap();
        peers
            .read()
            .await
            .get(&PeerKey::from_slice(dest_ip_slice))
            .map(PeerVal::forward_socket_addr)
    }
}
