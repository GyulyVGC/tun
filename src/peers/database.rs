use crate::peers::peer::Peer;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_rusqlite::Connection;

const SQLITE_PATH: &str = "./peers.sqlite";

pub enum PeerDbAction {
    Insert,
    Modify,
    Remove,
}

pub async fn manage_db(mut rx: UnboundedReceiver<(Peer, PeerDbAction)>) {
    let connection = Connection::open(SQLITE_PATH).await.unwrap();

    // make sure peer table exists and it's empty
    setup_db(&connection).await;

    // listen for messages on the channel
    if let Some((peer, action)) = rx.recv().await {
        match action {
            PeerDbAction::Insert => insert_peer(&connection, peer).await,
            PeerDbAction::Modify => modify_peer(&connection, peer).await,
            PeerDbAction::Remove => remove_peer(&connection, peer).await,
        }
    }
}

async fn insert_peer(connection: &Connection, peer: Peer) {
    let Peer { key, val } = peer;
    connection
        .call(move |c| {
            c.execute(
                "INSERT INTO peer (tun_ip, eth_ip, avg_delay, num_seen_unicast, num_seen_multicast, last_seen)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (key.tun_ip.to_string(), val.eth_ip.to_string(), val.avg_delay_as_seconds(),
                val.num_seen_unicast, val.num_seen_multicast, val.last_seen.to_string()),
            ).unwrap();
            Ok(())
        })
        .await
        .unwrap();
}

async fn modify_peer(connection: &Connection, peer: Peer) {
    let Peer { key, val } = peer;
    connection
        .call(move |c| {
            c.execute(
                "UPDATE peer
                    SET eth_ip = ?1,
                        avg_delay = ?2,
                        num_seen_unicast = ?3,
                        num_seen_multicast = ?4,
                        last_seen = ?5
                    WHERE tun_ip = ?6",
                (
                    val.eth_ip.to_string(),
                    val.avg_delay_as_seconds(),
                    val.num_seen_unicast,
                    val.num_seen_multicast,
                    val.last_seen.to_string(),
                    key.tun_ip.to_string(),
                ),
            )
            .unwrap();
            Ok(())
        })
        .await
        .unwrap();
}

async fn remove_peer(connection: &Connection, peer: Peer) {
    let Peer { key, val: _ } = peer;
    connection
        .call(move |c| {
            c.execute(
                "DELETE FROM peer
                    WHERE tun_ip = ?1",
                [key.tun_ip.to_string()],
            )
            .unwrap();
            Ok(())
        })
        .await
        .unwrap();
}

async fn setup_db(connection: &Connection) {
    drop_table(connection).await;
    create_table(connection).await;
}

async fn drop_table<'a>(connection: &Connection) {
    connection
        .call(|c| {
            c.execute("DROP TABLE IF EXISTS peer", ()).unwrap();
            Ok(())
        })
        .await
        .unwrap();
}

async fn create_table(connection: &Connection) {
    connection
        .call(|c| {
            c.execute(
                "CREATE TABLE IF NOT EXISTS peer (
                        tun_ip             TEXT PRIMARY KEY NOT NULL,
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
