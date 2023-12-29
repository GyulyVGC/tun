use std::io;
use std::io::Write;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tun::platform::posix::Writer;

pub async fn receive(mut device: Writer, socket: Arc<UdpSocket>) -> io::Result<()> {
    let mut buf = [0; 4096];
    loop {
        // wait until there is an incoming packet on the socket
        if let Ok((num_bytes, from)) = socket.recv_from(&mut buf).await {
            // write packet to the kernel
            if num_bytes > 0 {
                device.write_all(&buf[..num_bytes]).unwrap_or(());
                println!("IN from {}:\n{:?}\n", from, &buf[..num_bytes]);
            }
        }
    }
}
