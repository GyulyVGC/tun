use std::net::IpAddr;
use std::sync::Arc;

use nullnet_firewall::{Firewall, FirewallAction, FirewallDirection};
use tokio::io::{AsyncWriteExt, WriteHalf};
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, RwLock};
use tun::AsyncDevice;

use crate::craft::reject_payloads::send_termination_message;
use crate::forward::frame::Frame;

/// Handles incoming network packets (receives packets from the socket and sends them to the TUN interface),
/// ensuring the firewall rules are correctly observed.
pub async fn receive(
    device: &Arc<Mutex<WriteHalf<AsyncDevice>>>,
    socket: &Arc<UdpSocket>,
    firewall: &Arc<RwLock<Firewall>>,
    tun_ip: &IpAddr,
) {
    let mut frame = Frame::new();
    let mut remote_socket;
    loop {
        // wait until there is an incoming packet on the socket (packets on the socket are raw IP)
        let Ok((s, r)) = socket.recv_from(&mut frame.frame).await else {
            continue;
        };
        (frame.size, remote_socket) = (s, r);

        if frame.size > 0 {
            let pkt_data = frame.pkt_data();
            match firewall
                .read()
                .await
                .resolve_packet(pkt_data, FirewallDirection::IN)
            {
                FirewallAction::ACCEPT => {
                    // write packet to the kernel
                    device.lock().await.write_all(pkt_data).await.unwrap_or(());
                }
                FirewallAction::REJECT => {
                    send_termination_message(pkt_data, tun_ip, socket, remote_socket).await;
                }
                FirewallAction::DENY => {}
            }
        }
    }
}
