use etherparse::{EtherType, LaxPacketHeaders, NetHeaders};
use nullnet_firewall::{Firewall, FirewallAction, FirewallDirection};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tun_rs::AsyncDevice;

use crate::forward::frame::Frame;
use crate::peers::peer::{Peers, VethKey};

/// Handles outgoing network packets (receives packets from the TUN interface and sends them to the socket),
/// ensuring the firewall rules are correctly observed.
pub async fn send(
    device: &Arc<AsyncDevice>,
    socket: &Arc<UdpSocket>,
    firewall: &Arc<RwLock<Firewall>>,
    peers: Arc<RwLock<Peers>>,
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

async fn get_dst_socket(pkt_data: &[u8], peers: &Arc<RwLock<Peers>>) -> Option<SocketAddr> {
    let headers = LaxPacketHeaders::from_ethernet(pkt_data).ok()?;
    let vlan_id = headers.vlan_ids().first().map(|v| v.value())?;
    let dest_ip_slice = match headers.net {
        Some(NetHeaders::Ipv4(ipv4_header, _)) => Some(ipv4_header.destination),
        Some(NetHeaders::Arp(arp_packet)) => match arp_packet.proto_addr_type {
            EtherType::IPV4 => TryInto::<[u8; 4]>::try_into(arp_packet.target_protocol_addr()).ok(),
            _ => None,
        },
        _ => None,
    }?;
    let dest_ip = Ipv4Addr::from(dest_ip_slice);
    let veth_key = VethKey::new(dest_ip, vlan_id);

    peers.read().await.get_socket_by_veth(&veth_key)
}
