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
    let mut buf = [0; 4096];
    loop {
        // read, blocking until a packet is available from the kernel
        let num_bytes = device.read(&mut buf).unwrap_or(0);
        // send the packet to the socket
        if num_bytes > 0 {
            socket
                .send_to(&buf[..num_bytes], dst_socket_address)
                .await?;
            println!("OUT to {}\n\t{:?}\n", dst_socket_address, &buf[..num_bytes]);
        }
    }
}
