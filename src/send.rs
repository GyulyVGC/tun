use std::io;
use std::io::Read;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tun::platform::posix::Reader;

pub async fn send(
    mut device: Reader,
    socket: Arc<UdpSocket>,
    dst_socket_address: SocketAddr,
) -> io::Result<()> {
    let mut buf_os = [0; 4096];
    loop {
        // wait until there is a packet outgoing from kernel
        let num_bytes = device.read(&mut buf_os).unwrap_or(0);
        // send the packet to the socket
        if num_bytes > 0 {
            #[cfg(not(target_os = "macos"))]
            let buf_socket = &buf_os[..num_bytes];
            #[cfg(target_os = "macos")]
            let buf_socket = &buf_os[4..num_bytes];

            println!("OUT to {dst_socket_address}:\n{buf_socket:?}\n");

            socket
                .send_to(buf_socket, dst_socket_address)
                .await
                .unwrap_or(0);
        }
    }
}
