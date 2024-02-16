use crate::peers::peer::{PeerKey, PeerVal};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard, RwLock};
use tokio_rusqlite::Connection;

pub async fn update_db(
    db: &Arc<Mutex<Connection>>,
    peers: &Arc<RwLock<HashMap<PeerKey, PeerVal>>>,
) {
    let connection = db.lock().await;
    drop_table(&connection).await;
    create_table(&connection).await;
    update_table(&connection, peers).await;
}

async fn create_table<'a>(connection: &MutexGuard<'a, Connection>) {
    connection
        .call(|c| {
            c.execute(
                "CREATE TABLE IF NOT EXISTS peer (
                        tun_ip             TEXT PRIMARY KEY,
                        eth_ip             TEXT NOT NULL,
                        avg_delay          REAL NOT NULL,
                        num_seen_unicast   INTEGER NOT NULL,
                        num_seen_multicast INTEGER NOT NULL,
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
                     peer_val.num_seen_multicast, peer_val.avg_delay_as_seconds(), peer_val.last_seen.to_string()),
                ).unwrap();
                Ok(())
            })
            .await
            .unwrap();
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
