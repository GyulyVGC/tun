use listeners::Listener;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use std::fmt::{Display, Formatter};
use std::net::IpAddr;
use tokio_rusqlite::types::ToSqlOutput;
use tokio_rusqlite::ToSql;

#[derive(Serialize, Deserialize, PartialEq, Debug, Default, Clone)]
pub struct TunListenersAll {
    listeners: BTreeSet<TunListener>,
}

impl TunListenersAll {
    pub fn from_listeners(listeners: HashSet<Listener>, addr: IpAddr) -> Self {
        Self {
            listeners: listeners
                .into_iter()
                .filter(|listener| {
                    listener.socket.ip() == addr || listener.socket.ip().is_unspecified()
                })
                .map(TunListener::from_listener)
                .collect(),
        }
    }
}

impl Display for TunListenersAll {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let string = self
            .listeners
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<String>>()
            .join(", ");

        write!(f, "[{string}]")
    }
}

impl ToSql for TunListenersAll {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.to_string().into())
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Default, Clone, Eq, Ord, PartialOrd)]
struct TunListener {
    pid: u32,
    name: String,
    port: u16,
    // #[serde(rename = "listeners")]
    // pub(crate) names: BTreeSet<String>,
}

impl TunListener {
    fn from_listener(listener: Listener) -> Self {
        Self {
            pid: listener.pid,
            name: listener.name,
            port: listener.socket.port(),
        }
    }
}

impl Display for TunListener {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{} on {}", self.name, self.pid, self.port)
    }
}

#[cfg(test)]
mod tests {
    use crate::peers::tun_listener::TunListener;
    use rusqlite::ToSql;
    use std::collections::BTreeSet;
    use tokio_rusqlite::types::ToSqlOutput;
    use tokio_rusqlite::types::Value::Text;

    #[test]
    fn test_listener_processes_to_sql() {
        let listener_processes = TunListener {
            names: BTreeSet::from([
                "nullnet".to_string(),
                "nullnetd".to_string(),
                "tun".to_string(),
            ]),
        };
        assert_eq!(
            TunListener::to_sql(&listener_processes),
            Ok(ToSqlOutput::Owned(Text(
                "[nullnet, nullnetd, tun]".to_string()
            )))
        );
    }
}
