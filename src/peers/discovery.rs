use crate::local_endpoints::LocalEndpoints;
use crate::peers::hello::Hello;
use crate::peers::local_ips::LocalIps;
use crate::peers::peer::{PeerKey, PeerVal};
use chrono::Utc;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, MutexGuard, RwLock};
use tokio_rusqlite::Connection;

const RETRIES: u64 = 4;

// values in seconds
const TTL: u64 = 60; // * 60;
const RETRANSMISSION_PERIOD: u64 = TTL / 4;
const RETRIES_DELTA: u64 = 1;

const SQLITE_PATH: &str = "./peers.sqlite";

pub async fn discover_peers(
    endpoints: LocalEndpoints,
    peers: Arc<RwLock<HashMap<PeerKey, PeerVal>>>,
) {
    let socket = endpoints.sockets.discovery;
    let socket_2 = socket.clone();
    let socket_3 = socket_2.clone();
    let multicast_socket = endpoints.sockets.discovery_multicast;

    let multicast_socket_addr = multicast_socket.local_addr().unwrap();

    let local_ips = endpoints.ips.clone();
    let local_ips_2 = local_ips.clone();

    let peers_2 = peers.clone();
    let peers_3 = peers.clone();

    let db = Arc::new(Mutex::new(Connection::open(SQLITE_PATH).await.unwrap()));
    let db_2 = db.clone();
    let db_3 = db.clone();

    // listen for multicast hello messages
    tokio::spawn(async move {
        listen(
            ListenType::Multicast(socket_3),
            multicast_socket,
            local_ips,
            peers,
            db,
        )
        .await;
    });

    // listen for unicast hello responses
    tokio::spawn(async move {
        listen(ListenType::Unicast, socket, local_ips_2, peers_2, db_2).await;
    });

    // remove inactive peers
    tokio::spawn(async move {
        remove_inactive_peers(peers_3, db_3).await;
    });

    // periodically send out multicast hello messages
    greet_multicast(socket_2, multicast_socket_addr, endpoints.ips).await;
}

/// Listens to hello messages, updates the peers file, and invokes `greet_unicast` when needed.
async fn listen(
    listen_type: ListenType,
    listen_socket: Arc<UdpSocket>,
    local_ips: LocalIps,
    peers: Arc<RwLock<HashMap<PeerKey, PeerVal>>>,
    db: Arc<Mutex<Connection>>,
) {
    // used to determine whether a unicast response is required
    let mut should_respond_to;
    let mut msg = [0; 256];
    loop {
        should_respond_to = None;

        let (msg_len, from) = listen_socket
            .recv_from(&mut msg)
            .await
            .unwrap_or_else(|_| (0, SocketAddr::from_str("0.0.0.0:0").unwrap()));

        let now = Utc::now();
        let hello = Hello::from_toml_bytes(&msg[0..msg_len]);

        if !hello.is_valid(&from, &local_ips) {
            continue;
        };

        let delay = (now - hello.timestamp).num_microseconds().unwrap();

        peers
            .write()
            .await
            .entry(PeerKey::from_ip_addr(hello.ips.tun))
            .and_modify(|peer| {
                let since_last_seen = (now - peer.last_seen).num_seconds().unsigned_abs();
                if hello.is_setup && since_last_seen > RETRIES * RETRIES_DELTA {
                    should_respond_to = Some(peer.discovery_socket_addr());
                }
                peer.refresh(delay, &hello, listen_type.is_unicast());
            })
            .or_insert_with(|| {
                let peer = PeerVal::with_details(delay, &hello, listen_type.is_unicast());
                should_respond_to = Some(peer.discovery_socket_addr());
                peer
            });

        if let Some(dest_socket_addr) = should_respond_to {
            if let ListenType::Multicast(socket) = listen_type.clone() {
                let local_ips_2 = local_ips.clone();
                tokio::spawn(async move {
                    greet_unicast(socket, dest_socket_addr, local_ips_2).await;
                });
            }
        }

        update_db(&db, &peers).await;
    }
}

/// Checks for peers inactive for longer than `TTL` seconds and removes them from the peers list.
async fn remove_inactive_peers(
    peers: Arc<RwLock<HashMap<PeerKey, PeerVal>>>,
    db: Arc<Mutex<Connection>>,
) {
    loop {
        let oldest_peer = peers
            .read()
            .await
            .values()
            .min_by(|p1, p2| p1.last_seen.cmp(&p2.last_seen))
            .cloned();
        let sleep_time = if let Some(p) = oldest_peer {
            // TODO: timestamps must be monotonic!
            let since_oldest = (Utc::now() - p.last_seen).num_seconds().unsigned_abs();
            TTL.checked_sub(since_oldest).unwrap_or_default()
        } else {
            TTL
        };

        tokio::time::sleep(Duration::from_secs(sleep_time)).await;

        peers
            .write()
            .await
            .retain(|_, p| (Utc::now() - p.last_seen).num_seconds().unsigned_abs() < TTL);

        update_db(&db, &peers).await;
    }
}

/// Periodically sends out messages to let all other peers know that this device is up.
async fn greet_multicast(socket: Arc<UdpSocket>, dest: SocketAddr, local_ips: LocalIps) {
    // require unicast responses when this peer first joins the network
    let mut is_setup = true;
    loop {
        greet(&socket, dest, &local_ips, is_setup).await;
        is_setup = false;
        tokio::time::sleep(Duration::from_secs(RETRANSMISSION_PERIOD)).await;
    }
}

/// Sends out messages to acknowledge a specific peer that this device is up.
async fn greet_unicast(socket: Arc<UdpSocket>, dest: SocketAddr, local_ips: LocalIps) {
    greet(&socket, dest, &local_ips, false).await;
}

/// Sends out replicated hello messages to multicast or to a specific peer.
async fn greet(socket: &Arc<UdpSocket>, dest: SocketAddr, local_ips: &LocalIps, is_setup: bool) {
    for _ in 0..RETRIES {
        socket
            .send_to(
                Hello::with_details(local_ips, is_setup)
                    .to_toml_string()
                    .as_bytes(),
                dest,
            )
            .await
            .unwrap_or(0);
        tokio::time::sleep(Duration::from_secs(RETRIES_DELTA)).await;
    }
}

async fn drop_table<'a>(connection: &MutexGuard<'a, Connection>) {
    connection
        .call(|c| {
            c.execute("DROP TABLE IF EXISTS peer", ()).unwrap();
            Ok(())
        })
        .await
        .unwrap();
}

async fn create_table<'a>(connection: &MutexGuard<'a, Connection>) {
    connection
        .call(|c| {
            c.execute(
                "CREATE TABLE IF NOT EXISTS peer (
                        tun_ip             TEXT PRIMARY KEY,
                        eth_ip             TEXT NOT NULL,
                        num_seen_unicast   INTEGER NOT NULL,
                        num_seen_multicast INTEGER NOT NULL,
                        avg_delay          REAL NOT NULL,
                        last_seen          TEXT NOT NULL
                    )",
                (),
            )
            .unwrap();
            Ok(())
        })
        .await
        .unwrap();
}

async fn update_table<'a>(
    connection: &MutexGuard<'a, Connection>,
    peers: &Arc<RwLock<HashMap<PeerKey, PeerVal>>>,
) {
    for (peer_key, peer_val) in peers.read().await.iter() {
        let (peer_key, peer_val) = (peer_key.to_owned(), peer_val.to_owned());
        connection
        .call(move |c| {
                c.execute(
                    "INSERT INTO peer (tun_ip, eth_ip, num_seen_unicast, num_seen_multicast, avg_delay, last_seen)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    (peer_key.tun_ip.to_string(), peer_val.eth_ip.to_string(), peer_val.num_seen_unicast,
                     peer_val.num_seen_multicast, peer_val.avg_delay as f64 / 1_000_000_f64, peer_val.last_seen.to_string()),
                ).unwrap();
            Ok(())
        })
        .await
        .unwrap();
    }
}

async fn update_db(db: &Arc<Mutex<Connection>>, peers: &Arc<RwLock<HashMap<PeerKey, PeerVal>>>) {
    let connection = db.lock().await;
    drop_table(&connection).await;
    create_table(&connection).await;
    update_table(&connection, peers).await;
}

#[derive(Clone)]
enum ListenType {
    /// Listen for unicast hello messages.
    Unicast,
    /// Listen for multicast hello messages, and send out unicast responses when needed from the associated object.
    Multicast(Arc<UdpSocket>),
}

impl ListenType {
    pub fn is_unicast(&self) -> bool {
        matches!(self, Self::Unicast)
    }
}
