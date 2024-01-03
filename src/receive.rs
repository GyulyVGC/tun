use crate::socket_frame::SocketFrame;
use std::io::Write;
use std::net::{SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tun::platform::posix::Writer;

pub async fn receive(mut device: Writer, socket: Arc<UdpSocket>) {
    let mut socket_frame = SocketFrame::new();
    loop {
        // wait until there is an incoming packet on the socket (packets on the socket are raw IP)
        (socket_frame.actual_bytes, _) = socket
            .recv_from(&mut socket_frame.frame)
            .await
            .unwrap_or((0, SocketAddr::from_str("0.0.0.0:0").unwrap()));

        // write packet to the kernel
        if socket_frame.actual_bytes > 0 {
            // let Some(src_tun_ip) = get_src_tun_ip(socket_frame.actual_frame()) else {
            //     continue;
            // };
            // println!("IN from {src_tun_ip}:\n{:?}\n", socket_frame.actual_frame());

            let os_buf = socket_frame.to_os_buf();

            #[allow(clippy::needless_borrow)]
            device.write_all(&os_buf).unwrap_or(());
        }
    }
}

// fn get_src_tun_ip(socket_buf: &[u8]) -> Option<IpAddr> {
//     if socket_buf.len() < 20 {
//         None
//     } else {
//         let mut src_tun_ip_octects = [0; 4];
//         src_tun_ip_octects.clone_from_slice(&socket_buf[12..16]);
//         Some(IpAddr::from(src_tun_ip_octects))
//     }
// }
