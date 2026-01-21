use crate::peers::local_ips::LocalIps;
use crate::peers::peer::Peers;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;

/// Listens to hello messages (unicast or broadcast), and invokes `greet_unicast` when needed.
async fn listen(
    listen_socket: Arc<UdpSocket>,
    unicast_socket: Arc<UdpSocket>,
    local_ips: LocalIps,
    peers: Arc<RwLock<Peers>>,
) {
    //TODO: listen on the gRPC control channel:
    // - setup VLAN if target IP is this machine
    // - register peer if target IP is another machine
}

// /// Periodically sends out messages to let all other peers know that this device is up.
// async fn greet_broadcast(socket: Arc<UdpSocket>, local_ips: LocalIps) {
//     // require unicast responses when this peer first joins the network
//     let mut is_setup = true;
//     let dest = SocketAddr::new(IpAddr::V4(local_ips.ethernet.broadcast), DISCOVERY_PORT);
//     loop {
//         greet(&socket, dest, &local_ips, is_setup, true, false).await;
//         is_setup = false;
//         tokio::time::sleep(Duration::from_secs(RETRANSMISSION_PERIOD)).await;
//     }
// }
