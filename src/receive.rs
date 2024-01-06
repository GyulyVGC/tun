use crate::socket_frame::SocketFrame;
use std::io::Write;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;
use tun::platform::posix::Writer;

pub fn receive(mut device: Writer, socket: &Arc<UdpSocket>) {
    let mut socket_frame = SocketFrame::new();
    loop {
        let mut inst = Instant::now();
        // wait until there is an incoming packet on the socket (packets on the socket are raw IP)
        (socket_frame.actual_bytes, _) = socket
            .recv_from(&mut socket_frame.frame)
            .unwrap_or_else(|_| (0, SocketAddr::from_str("0.0.0.0:0").unwrap()));

        // println!("RXA {}", inst.elapsed().as_micros());
        inst = Instant::now();

        if socket_frame.actual_bytes > 0 {
            // write packet to the kernel
            let os_buf = socket_frame.to_os_buf();
            #[allow(clippy::needless_borrow)]
            device.write_all(&os_buf).unwrap_or(());
        }
        // println!("RXB {}", inst.elapsed().as_micros());
    }
}
