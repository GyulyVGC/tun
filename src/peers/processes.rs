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
/// The set of processes listening on a TCP port.
pub struct Processes(BTreeSet<Process>);

impl Processes {
    pub fn from_listeners(listeners: HashSet<Listener>, addr: IpAddr) -> Self {
        Self(
            listeners
                .into_iter()
                .filter(|listener| {
                    let ip = listener.socket.ip();
                    ip.is_ipv4() && (ip == addr || ip.is_unspecified())
                })
                .map(Process::from_listener)
                .collect(),
        )
    }
}

impl Display for Processes {
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

impl FromStr for Processes {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err_str = "Wrong format for processes collection";
        let processes = s
            .strip_prefix('[')
            .ok_or(err_str)?
            .strip_suffix(']')
            .ok_or(err_str)?
            .split(", ")
            .filter(|str| !str.is_empty())
            .map(|str| str.parse().map_err(|e: String| e))
            .collect::<Result<BTreeSet<Process>, String>>()?;
        Ok(Self(processes))
    }
}

impl ToSql for Processes {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.to_string().into())
    }
}

impl Serialize for Processes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Processes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let string = String::deserialize(deserializer)?;
        let processes = string
            .parse()
            .map_err(|e: String| Error::custom(e.to_string()))?;
        Ok(processes)
    }
}

#[derive(PartialEq, Debug, Default, Clone, Eq, Ord, PartialOrd)]
/// A process listening on a TCP port.
struct Process {
    pid: u32,
    name: String,
    port: u16,
}

impl Process {
    fn from_listener(listener: Listener) -> Self {
        Self {
            pid: listener.pid,
            name: listener.name,
            port: listener.socket.port(),
        }
    }
}

impl Display for Process {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{} on {}", self.pid, self.name, self.port)
    }
}

impl FromStr for Process {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err_str = "Wrong format for process";

        let mut pid_other = s.splitn(2, '/');
        let pid = pid_other
            .next()
            .ok_or(err_str)?
            .parse()
            .map_err(|e: ParseIntError| e.to_string())?;
        let name_port = pid_other.next().ok_or(err_str)?;

        let mut parts = name_port.rsplitn(2, " on ");
        let port = parts
            .next()
            .ok_or(err_str)?
            .parse()
            .map_err(|e: ParseIntError| e.to_string())?;
        let name = parts.next().ok_or(err_str)?.to_string();

        Ok(Self { pid, name, port })
    }
}

#[cfg(test)]
mod tests {
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

    use crate::peers::processes::Process;

    #[test]
    fn test_process_to_string() {
        let process = Process {
            pid: 1234,
            name: "nullnet".to_string(),
            port: 80,
        };
        assert_eq!(process.to_string(), "1234/nullnet on 80");
    }

    #[test]
    fn test_process_from_string() {
        let process = "1234/nullnet on 80".parse();
        assert_eq!(
            process,
            Ok(Process {
                pid: 1234,
                name: "nullnet".to_string(),
                port: 80,
            })
        );

        let process = "1234/nullnet/daemon on 80".parse();
        assert_eq!(
            process,
            Ok(Process {
                pid: 1234,
                name: "nullnet/daemon".to_string(),
                port: 80,
            })
        );

        let process = "1234/nullnet-daem on  on 80".parse();
        assert_eq!(
            process,
            Ok(Process {
                pid: 1234,
                name: "nullnet-daem on ".to_string(),
                port: 80,
            })
        );

        let process = "1234/nullnet on  80".to_string().parse::<Process>();
        assert_eq!(process, Err("invalid digit found in string".to_string()));

        let process = "999/nullnet on 65536".to_string().parse::<Process>();
        assert_eq!(
            process,
            Err("number too large to fit in target type".to_string())
        );

        let process = "1234 /nullnet on 80".to_string().parse::<Process>();
        assert_eq!(process, Err("invalid digit found in string".to_string()));

        let process = "1234-nullnet on 80".to_string().parse::<Process>();
        assert_eq!(process, Err("invalid digit found in string".to_string()));
    }
}
