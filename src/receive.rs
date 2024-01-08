use crate::socket_frame::SocketFrame;
use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::aead::Aead;
use aes_gcm::Aes256Gcm;
use nullnet_firewall::{Firewall, FirewallAction, FirewallDirection};
use std::io::Write;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use tun::platform::posix::Writer;

pub fn receive(
    mut device: Writer,
    socket: &Arc<UdpSocket>,
    firewall: &Arc<RwLock<Firewall>>,
    cipher: &Arc<Aes256Gcm>,
) {
    let mut socket_frame = SocketFrame::new();
    loop {
        // wait until there is an incoming packet on the socket (packets on the socket are raw IP)
        (socket_frame.actual_bytes, _) = socket
            .recv_from(&mut socket_frame.frame)
            .unwrap_or_else(|_| (0, SocketAddr::from_str("0.0.0.0:0").unwrap()));

        if socket_frame.actual_bytes > 0 {
            match firewall
                .read()
                .unwrap()
                .resolve_packet(socket_frame.actual_frame(), FirewallDirection::IN)
            {
                FirewallAction::ACCEPT => {
                    // write packet to the kernel
                    let decrypted = decrypt_packet(socket_frame.actual_frame(), cipher);
                    // let os_buf = socket_frame.to_os_buf();
                    #[allow(clippy::needless_borrow)]
                    device.write_all(decrypted.as_slice()).unwrap_or(());
                }
                FirewallAction::DENY | FirewallAction::REJECT => {}
            }
        }
    }
}

fn decrypt_packet(packet: &[u8], cipher: &Arc<Aes256Gcm>) -> Vec<u8> {
    let nonce = GenericArray::from_slice(&[0; 12]);
    cipher.decrypt(nonce, packet).unwrap_or(vec![])
}
