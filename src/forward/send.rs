use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use nullnet_firewall::{Firewall, FirewallAction, FirewallDirection};
use tokio::io::{AsyncReadExt, ReadHalf};
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, RwLock};
use tun2::AsyncDevice;

use crate::forward::frame::Frame;
use crate::peers::peer::{PeerKey, PeerVal};

/// Handles outgoing network packets (receives packets from the TUN interface and sends them to the socket),
/// ensuring the firewall rules are correctly observed.
pub async fn send(
    device: &Arc<Mutex<ReadHalf<AsyncDevice>>>,
    socket: &Arc<UdpSocket>,
    firewall: &Arc<RwLock<Firewall>>,
    peers: Arc<RwLock<HashMap<PeerKey, PeerVal>>>,
) {
    let mut frame = Frame::new();
    loop {
        // wait until there is a packet outgoing from kernel
        frame.size = device
            .lock()
            .await
            .read(&mut frame.frame)
            .await
            .unwrap_or(0);

        if frame.size > 0 {
            // send the packet to the socket
            let pkt_data = frame.pkt_data();
            let Some(dst_socket) = get_dst_socket(pkt_data, &peers).await else {
                continue;
            };
            match firewall
                .read()
                .await
                .resolve_packet(pkt_data, FirewallDirection::OUT)
            {
                FirewallAction::ACCEPT => {
                    socket.send_to(pkt_data, dst_socket).await.unwrap_or(0);
                }
                FirewallAction::DENY | FirewallAction::REJECT => {}
            };
        }
    }
}

async fn get_dst_socket(
    pkt_data: &[u8],
    peers: &Arc<RwLock<HashMap<PeerKey, PeerVal>>>,
) -> Option<SocketAddr> {
    if pkt_data.len() < 20 {
        None
    } else {
        let dest_ip_slice: [u8; 4] = pkt_data[16..20].try_into().unwrap();
        peers
            .read()
            .await
            .get(&PeerKey::from_slice(dest_ip_slice))
            .map(PeerVal::forward_socket_addr)
    }
}
