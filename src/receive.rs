use crate::socket_frame::SocketFrame;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, WriteHalf};
use tokio::net::UdpSocket;
use tun::AsyncQueue;

pub async fn receive(mut queue: WriteHalf<AsyncQueue>, socket: Arc<UdpSocket>, i: usize) {
    let mut socket_frame = SocketFrame::new();
    loop {
        // wait until there is an incoming packet on the socket (packets on the socket are raw IP)
        (socket_frame.actual_bytes, _) = socket
            .recv_from(&mut socket_frame.frame)
            .await
            .unwrap_or_else(|_| (0, SocketAddr::from_str("0.0.0.0:0").unwrap()));

        println!("RX {i}");

        // write packet to the kernel
        if socket_frame.actual_bytes > 0 {
            // let Some(src_tun_ip) = get_src_tun_ip(socket_frame.actual_frame()) else {
            //     continue;
            // };
            // println!("IN from {src_tun_ip}:\n{:?}\n", socket_frame.actual_frame());

            let os_buf = socket_frame.to_os_buf();

            #[allow(clippy::needless_borrow)]
            queue.write_all(&os_buf).await.unwrap_or(());

            println!("--- RX {i}");
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
