use crate::peers::peer::{PeerKey, PeerVal, VethKey};
use nullnet_liberror::{Error, ErrorHandler, Location, location};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_rusqlite::Connection;

const SQLITE_PATH: &str = "./peers.sqlite";

/// Struct representing a peer for storage in database.
pub struct PeerDbData {
    pub(crate) key: PeerKey,
    pub(crate) val: PeerVal,
    pub(crate) veths: Vec<VethKey>,
    pub(crate) action: PeerDbAction,
}

pub enum PeerDbAction {
    Insert,
    Modify,
    Remove,
}

/// Handles the peers database, receiving messages from the channel and sending proper queries to the DB.
pub async fn manage_peers_db(mut rx: UnboundedReceiver<PeerDbData>) -> Result<(), Error> {
    let connection = Connection::open(SQLITE_PATH)
        .await
        .handle_err(location!())?;

    // make sure tables exist and are empty
    setup_db(&connection).await?;

    // keep listening for messages on the channel
    loop {
        if let Some(peer) = rx.recv().await {
            match peer.action {
                PeerDbAction::Insert => insert_peer(&connection, peer).await?,
                PeerDbAction::Modify => modify_peer(&connection, peer).await?,
                PeerDbAction::Remove => remove_peer(&connection, peer).await?,
            }
        }
    }
}

/// Inserts a new entry into the peers DB.
async fn insert_peer(connection: &Connection, peer: PeerDbData) -> Result<(), Error> {
    let PeerDbData {
        key, val, veths, ..
    } = peer;
    let ethernet_ip = key.ethernet_ip.to_string();

    remove_veths_for_peer(connection, ethernet_ip.clone()).await?;
    insert_veths_for_peer(connection, ethernet_ip.clone(), veths).await?;

    connection
        .call(move |c| {
            let _ = c.execute(
                "INSERT INTO peers (ethernet_ip, avg_delay, num_seen_unicast, num_seen_broadcast, last_seen, processes)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (ethernet_ip, val.avg_delay_as_seconds(),
                 val.num_seen_unicast, val.num_seen_broadcast, val.last_seen.to_string(), val.processes),
            ).handle_err(location!());
            Ok(())
        })
        .await
        .handle_err(location!())?;

    Ok(())
}

/// Modifies an existing entry in the peers DB.
async fn modify_peer(connection: &Connection, peer: PeerDbData) -> Result<(), Error> {
    let PeerDbData {
        key, val, veths, ..
    } = peer;
    let ethernet_ip = key.ethernet_ip.to_string();

    remove_veths_for_peer(connection, ethernet_ip.clone()).await?;
    insert_veths_for_peer(connection, ethernet_ip.clone(), veths).await?;

    connection
        .call(move |c| {
            let _ = c
                .execute(
                    "UPDATE peers
                    SET avg_delay = ?1,
                        num_seen_unicast = ?2,
                        num_seen_broadcast = ?3,
                        last_seen = ?4,
                        processes = ?5
                    WHERE ethernet_ip = ?6",
                    (
                        val.avg_delay_as_seconds(),
                        val.num_seen_unicast,
                        val.num_seen_broadcast,
                        val.last_seen.to_string(),
                        val.processes,
                        ethernet_ip,
                    ),
                )
                .handle_err(location!());
            Ok(())
        })
        .await
        .handle_err(location!())?;

    Ok(())
}

/// Removes an entry from the peers DB.
async fn remove_peer(connection: &Connection, peer: PeerDbData) -> Result<(), Error> {
    let PeerDbData { key, .. } = peer;
    let ethernet_ip = key.ethernet_ip.to_string();

    remove_veths_for_peer(connection, ethernet_ip.clone()).await?;

    connection
        .call(move |c| {
            let _ = c
                .execute(
                    "DELETE FROM peers
                    WHERE ethernet_ip = ?1",
                    [ethernet_ip],
                )
                .handle_err(location!());
            Ok(())
        })
        .await
        .handle_err(location!())?;

    Ok(())
}

async fn remove_veths_for_peer(connection: &Connection, ethernet_ip: String) -> Result<(), Error> {
    connection
        .call(move |c| {
            let _ = c
                .execute(
                    "DELETE FROM veths
                    WHERE ethernet_ip = ?1",
                    [ethernet_ip],
                )
                .handle_err(location!());
            Ok(())
        })
        .await
        .handle_err(location!())?;

    Ok(())
}

async fn insert_veths_for_peer(
    connection: &Connection,
    ethernet_ip: String,
    veths: Vec<VethKey>,
) -> Result<(), Error> {
    connection
        .call(move |c| {
            let Ok(tran) = c.transaction().handle_err(location!()) else {
                return Ok(());
            };
            for veth in veths {
                let _ = tran
                    .execute(
                        "INSERT INTO veths (veth_ip, vlan_id, ethernet_ip)
                        VALUES (?1, ?2, ?3)",
                        (veth.veth_ip.to_string(), veth.vlan_id, ethernet_ip.clone()),
                    )
                    .handle_err(location!());
            }
            let _ = tran.commit().handle_err(location!());
            Ok(())
        })
        .await
        .handle_err(location!())?;

    Ok(())
}

/// Drop the peers table and creates a new one.
async fn setup_db(connection: &Connection) -> Result<(), Error> {
    for name in ["peers", "veths"] {
        drop_table(connection, name).await?;
    }

    create_peers_table(connection).await?;
    create_veths_table(connection).await?;

    Ok(())
}

/// Drops the peers table.
async fn drop_table(connection: &Connection, name: &str) -> Result<(), Error> {
    let sql = format!("DROP TABLE IF EXISTS {name}");
    connection
        .call(move |c| {
            let _ = c.execute(&sql, ()).handle_err(location!());
            Ok(())
        })
        .await
        .handle_err(location!())?;

    Ok(())
}

/// Creates the peers table.
async fn create_peers_table(connection: &Connection) -> Result<(), Error> {
    connection
        .call(|c| {
            let _ = c
                .execute(
                    "CREATE TABLE IF NOT EXISTS peers (
                        ethernet_ip        TEXT PRIMARY KEY NOT NULL,
                        avg_delay          REAL NOT NULL,
                        num_seen_unicast   INTEGER NOT NULL,
                        num_seen_broadcast INTEGER NOT NULL,
                        last_seen          TEXT NOT NULL,
                        processes          TEXT NOT NULL
                    )",
                    (),
                )
                .handle_err(location!());
            Ok(())
        })
        .await
        .handle_err(location!())?;

    Ok(())
}

/// Creates the veths table.
async fn create_veths_table(connection: &Connection) -> Result<(), Error> {
    connection
        .call(|c| {
            let _ = c
                .execute(
                    "CREATE TABLE IF NOT EXISTS veths (
                        veth_ip            TEXT NOT NULL,
                        vlan_id            INTEGER NOT NULL,
                        ethernet_ip        TEXT,
                        PRIMARY KEY (veth_ip, vlan_id),
                        FOREIGN KEY (ethernet_ip) REFERENCES peers (ethernet_ip)
                    )",
                    (),
                )
                .handle_err(location!());
            Ok(())
        })
        .await
        .handle_err(location!())?;

    Ok(())
}
