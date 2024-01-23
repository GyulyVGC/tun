use crate::reject_payloads::send_termination_message;
use crate::socket_frame::SocketFrame;
use nullnet_firewall::{Firewall, FirewallAction, FirewallDirection};
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, WriteHalf};
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, RwLock};
use tun::AsyncDevice;

pub async fn receive(
    device: &Arc<Mutex<WriteHalf<AsyncDevice>>>,
    socket: &Arc<UdpSocket>,
    firewall: &Arc<RwLock<Firewall>>,
    tun_ip: &IpAddr,
) {
    let mut socket_frame = SocketFrame::new();
    loop {
        println!("receive");
        // wait until there is an incoming packet on the socket (packets on the socket are raw IP)
        (socket_frame.actual_bytes, _) = socket
            .recv_from(&mut socket_frame.frame)
            .await
            .unwrap_or_else(|_| (0, SocketAddr::from_str("0.0.0.0:0").unwrap()));

        if socket_frame.actual_bytes > 0 {
            match firewall
                .read()
                .await
                .resolve_packet(socket_frame.actual_frame(), FirewallDirection::IN)
            {
                FirewallAction::ACCEPT => {
                    // write packet to the kernel
                    let os_buf = socket_frame.to_os_buf();
                    #[allow(clippy::needless_borrow)]
                    device.lock().await.write_all(&os_buf).await.unwrap_or(());
                }
                FirewallAction::REJECT => {
                    send_termination_message(socket_frame.actual_frame(), tun_ip, socket).await;
                }
                FirewallAction::DENY => {}
            }
        }
    }
}
