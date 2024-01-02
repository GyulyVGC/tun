use crate::socket_frame::SocketFrame;
use std::io::Write;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tun::platform::posix::Writer;

pub async fn receive(mut device: Writer, socket: Arc<UdpSocket>) {
    let mut socket_frame = SocketFrame::new();
    loop {
        // wait until there is an incoming packet on the socket (packets on the socket are raw IP)
        if let Ok((num_bytes, from)) = socket.recv_from(&mut socket_frame.frame).await {
            socket_frame.actual_bytes = num_bytes;
            // write packet to the kernel
            if socket_frame.actual_bytes > 0 {
                println!("IN from {from}:\n{:?}\n", socket_frame.actual_frame());

                let os_buf = socket_frame.to_os_buf();

                device.write_all(&os_buf).unwrap_or(());
            }
        }
    }
}
