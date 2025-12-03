use etherparse::{LaxPacketHeaders, NetHeaders};
use nullnet_firewall::{Firewall, FirewallAction, FirewallDirection};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tun_rs::AsyncDevice;

use crate::forward::frame::Frame;
use crate::peers::peer::{PeerKey, PeerVal};

/// Handles outgoing network packets (receives packets from the TUN interface and sends them to the socket),
/// ensuring the firewall rules are correctly observed.
pub async fn send(
    device: &Arc<AsyncDevice>,
    socket: &Arc<UdpSocket>,
    firewall: &Arc<RwLock<Firewall>>,
    peers: Arc<RwLock<HashMap<PeerKey, PeerVal>>>,
) {
    let mut frame = Frame::new();
    loop {
        // wait until there is a packet outgoing from kernel
        frame.size = device.recv(&mut frame.frame).await.unwrap_or(0);

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
            }
        }
    }
}

async fn get_dst_socket(
    pkt_data: &[u8],
    peers: &Arc<RwLock<HashMap<PeerKey, PeerVal>>>,
) -> Option<SocketAddr> {
    peers
        .read()
        .await
        .iter()
        .next()
        .map(|(_, v)| v.forward_socket_addr())

    // TODO fix this
    // let headers = LaxPacketHeaders::from_ethernet(pkt_data).ok()?;
    // if let Some(NetHeaders::Ipv4(_ipv4_header, _)) = headers.net {
        // let dest_ip_slice = ipv4_header.destination;
        // peers
        //     .read()
        //     .await
        //     .get(&PeerKey::from_slice(dest_ip_slice))
        //     .map(PeerVal::forward_socket_addr)
    // } else {
    //     None
    // }
}
