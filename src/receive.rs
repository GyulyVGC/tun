use std::io;
use std::io::Write;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tun::platform::posix::Writer;

pub async fn receive(mut device: Writer, socket: Arc<UdpSocket>) -> io::Result<()> {
    let mut buf_socket = [0; 4096];
    loop {
        // wait until there is an incoming packet on the socket
        if let Ok((num_bytes, from)) = socket.recv_from(&mut buf_socket).await {
            // write packet to the kernel
            if num_bytes > 0 {
                println!("IN from {from}:\n{:?}\n", &buf_socket[..num_bytes]);

                #[cfg(not(target_os = "macos"))]
                let buf_os = &buf_socket[..num_bytes];
                #[cfg(target_os = "macos")]
                let buf_os: &[u8] = &[&[0, 0, 0, 2], &buf_socket[..num_bytes]].concat()[..];

                device.write_all(buf_os).unwrap_or(());
            }
        }
    }
}
