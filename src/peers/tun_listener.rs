use listeners::Listener;
use serde::de::Error;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use std::fmt::{Display, Formatter};
use std::net::IpAddr;
use std::num::ParseIntError;
use std::str::FromStr;
use tokio_rusqlite::types::ToSqlOutput;
use tokio_rusqlite::ToSql;

#[derive(PartialEq, Debug, Default, Clone)]
pub struct TunListenersAll(BTreeSet<TunListener>);

impl TunListenersAll {
    pub fn from_listeners(listeners: HashSet<Listener>, addr: IpAddr) -> Self {
        Self(
            listeners
                .into_iter()
                .filter(|listener| {
                    let ip = listener.socket.ip();
                    ip.is_ipv4() && (ip == addr || ip.is_unspecified())
                })
                .map(TunListener::from_listener)
                .collect(),
        )
    }
}

impl Display for TunListenersAll {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let string = self
            .0
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<String>>()
            .join(", ");

        write!(f, "[{string}]")
    }
}

impl FromStr for TunListenersAll {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err_str = "Wrong format for listeners collection";
        let listeners = s
            .strip_prefix('[')
            .ok_or(err_str)?
            .strip_suffix(']')
            .ok_or(err_str)?
            .split(", ")
            .filter(|str| !str.is_empty())
            .map(|str| str.parse().map_err(|e: String| e))
            .collect::<Result<BTreeSet<TunListener>, String>>()?;
        Ok(Self(listeners))
    }
}

impl ToSql for TunListenersAll {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.to_string().into())
    }
}

impl Serialize for TunListenersAll {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TunListenersAll {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let string = String::deserialize(deserializer)?;
        let listeners = string
            .parse()
            .map_err(|e: String| Error::custom(e.to_string()))?;
        Ok(listeners)
    }
}

#[derive(PartialEq, Debug, Default, Clone, Eq, Ord, PartialOrd)]
struct TunListener {
    pid: u32,
    name: String,
    port: u16,
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
        write!(f, "{}/{} on {}", self.pid, self.name, self.port)
    }
}

impl FromStr for TunListener {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err_str = "Wrong format for listener";
        let mut pid_other = s.split('/');
        let pid = pid_other
            .next()
            .ok_or(err_str)?
            .parse()
            .map_err(|e: ParseIntError| e.to_string())?;
        let name_port = pid_other.next().ok_or(err_str)?.to_string();
        let mut parts = name_port.split(" on ");
        let name = parts.next().ok_or(err_str)?.to_string();
        let port = parts
            .next()
            .ok_or(err_str)?
            .parse()
            .map_err(|e: ParseIntError| e.to_string())?;
        Ok(Self { pid, name, port })
    }
}

#[cfg(test)]
mod tests {
    // use crate::peers::tun_listener::TunListener;
    // use rusqlite::ToSql;
    // use std::collections::BTreeSet;
    // use tokio_rusqlite::types::ToSqlOutput;
    // use tokio_rusqlite::types::Value::Text;

    // #[test]
    // fn test_listener_processes_to_sql() {
    //     let listener_processes = TunListener {
    //         names: BTreeSet::from([
    //             "nullnet".to_string(),
    //             "nullnetd".to_string(),
    //             "tun".to_string(),
    //         ]),
    //     };
    //     assert_eq!(
    //         TunListener::to_sql(&listener_processes),
    //         Ok(ToSqlOutput::Owned(Text(
    //             "[nullnet, nullnetd, tun]".to_string()
    //         )))
    //     );
    // }
}
