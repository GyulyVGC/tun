use crate::socket_frame::SocketFrame;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncWriteExt, WriteHalf};
use tokio::net::UdpSocket;
use tun::AsyncDevice;

pub async fn receive(mut device: WriteHalf<AsyncDevice>, socket: Arc<UdpSocket>) {
    let mut socket_frame = SocketFrame::new();
    loop {
        let inst = Instant::now();
        // wait until there is an incoming packet on the socket (packets on the socket are raw IP)
        (socket_frame.actual_bytes, _) = socket
            .recv_from(&mut socket_frame.frame)
            .await
            .unwrap_or_else(|_| (0, SocketAddr::from_str("0.0.0.0:0").unwrap()));

        if socket_frame.actual_bytes > 0 {
            // write packet to the kernel
            let os_buf = socket_frame.to_os_buf();
            #[allow(clippy::needless_borrow)]
            device.write_all(&os_buf).await.unwrap_or(());
        }
        println!("RX {}", inst.elapsed().as_micros());
    }
}
