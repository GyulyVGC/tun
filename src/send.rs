use crate::os_frame::OsFrame;
use std::io::Read;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tun::platform::posix::Reader;

pub async fn send(mut device: Reader, socket: Arc<UdpSocket>, dst_socket_address: SocketAddr) {
    let mut os_frame = OsFrame::new();
    loop {
        // wait until there is a packet outgoing from kernel
        let num_bytes = device.read(&mut os_frame.frame).unwrap_or(0);
        os_frame.actual_bytes = num_bytes;
        // send the packet to the socket
        if os_frame.actual_bytes > 0 {
            let socket_buf = os_frame.to_socket_buf();

            println!("OUT to {dst_socket_address}:\n{socket_buf:?}\n");

            socket
                .send_to(socket_buf, dst_socket_address)
                .await
                .unwrap_or(0);
        }
    }
}
