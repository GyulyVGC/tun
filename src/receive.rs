use std::io;
use std::io::Write;
use std::sync::{Arc, Mutex};
use tokio::net::UdpSocket;
use tun::platform::Device;

pub async fn receive(device: Arc<Mutex<Device>>, socket: Arc<UdpSocket>) -> io::Result<()> {
    let mut buf = [0; 4096];
    loop {
        // wait until there is an incoming packet on the socket
        println!("before receive");
        let (num_bytes, from) = socket.recv_from(&mut buf).await?;
        println!("after receive");
        // write packet to the kernel
        if num_bytes > 0 {
            println!("before IN from {}\n\t{:?}\n", from, &buf[..num_bytes]);
            device.lock().unwrap().write_all(&buf[..num_bytes])?;
            println!("after IN from {}\n\t{:?}\n", from, &buf[..num_bytes]);
        }
    }
}
