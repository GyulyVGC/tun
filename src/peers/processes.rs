use std::collections::{BTreeSet, HashSet};
use std::fmt::{Display, Formatter};
use std::net::IpAddr;
use std::num::ParseIntError;
use std::str::FromStr;

use listeners::Listener;
use serde::de::Error;
use serde::{Deserialize, Serialize};
use tokio_rusqlite::ToSql;
use tokio_rusqlite::types::ToSqlOutput;

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
        let processes = string.parse().map_err(|e: String| Error::custom(e))?;
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
            pid: listener.process.pid,
            name: listener.process.name,
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
    use std::collections::{BTreeSet, HashSet};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

    use listeners::Listener;

    use crate::peers::processes::{Process, Processes};

    fn listeners_for_tests() -> HashSet<Listener> {
        let mut listeners = HashSet::new();
        listeners.insert(Listener {
            pid: 1,
            name: "nullnet".to_string(),
            socket: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 10),
        });
        listeners.insert(Listener {
            pid: 2,
            name: "tun".to_string(),
            socket: SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 20),
        });
        listeners.insert(Listener {
            pid: 3,
            name: "sshd".to_string(),
            socket: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 30),
        });
        listeners.insert(Listener {
            pid: 4,
            name: "tun".to_string(),
            socket: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)), 40),
        });
        listeners
    }

    #[test]
    fn test_process_from_listener() {
        let listener = Listener {
            pid: 1234,
            name: "nullnet".to_string(),
            socket: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 80),
        };
        let process = Process::from_listener(listener);
        assert_eq!(
            process,
            Process {
                pid: 1234,
                name: "nullnet".to_string(),
                port: 80,
            }
        );
    }

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

    #[test]
    fn test_processes_from_listeners_1() {
        let listeners = listeners_for_tests();
        let processes =
            Processes::from_listeners(listeners, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)));
        assert_eq!(
            processes,
            Processes(BTreeSet::from([
                Process {
                    pid: 1,
                    name: "nullnet".to_string(),
                    port: 10,
                },
                Process {
                    pid: 3,
                    name: "sshd".to_string(),
                    port: 30,
                },
            ]))
        );
    }

    #[test]
    fn test_processes_from_listeners_2() {
        let listeners = listeners_for_tests();
        let processes =
            Processes::from_listeners(listeners, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)));
        assert_eq!(
            processes,
            Processes(BTreeSet::from([
                Process {
                    pid: 1,
                    name: "nullnet".to_string(),
                    port: 10,
                },
                Process {
                    pid: 4,
                    name: "tun".to_string(),
                    port: 40,
                },
            ]))
        );
    }

    #[test]
    fn test_processes_to_string() {
        let processes = Processes(BTreeSet::from([
            Process {
                pid: 1,
                name: "nullnet".to_string(),
                port: 10,
            },
            Process {
                pid: 3,
                name: "sshd".to_string(),
                port: 30,
            },
        ]));
        assert_eq!(processes.to_string(), "[1/nullnet on 10, 3/sshd on 30]");
    }

    #[test]
    fn test_empty_processes_to_string() {
        let processes = Processes(BTreeSet::new());
        assert_eq!(processes.to_string(), "[]");
    }

    #[test]
    fn test_processes_from_string() {
        let processes = "[1/nullnet on 10, 3/sshd on 30]".parse();
        assert_eq!(
            processes,
            Ok(Processes(BTreeSet::from([
                Process {
                    pid: 1,
                    name: "nullnet".to_string(),
                    port: 10,
                },
                Process {
                    pid: 3,
                    name: "sshd".to_string(),
                    port: 30,
                },
            ])))
        );

        let processes = "[]".parse();
        assert_eq!(processes, Ok(Processes(BTreeSet::new())));

        let processes = "[1/nullnet on 10, 3/sshd on 30".parse::<Processes>();
        assert_eq!(
            processes,
            Err("Wrong format for processes collection".to_string())
        );

        let processes = "1/nullnet on 10, 3/sshd on 30]".parse::<Processes>();
        assert_eq!(
            processes,
            Err("Wrong format for processes collection".to_string())
        );

        let processes = "[1/nullnet on 10, 3/sshd on 30,]".parse::<Processes>();
        assert_eq!(processes, Err("invalid digit found in string".to_string()));

        let processes = "[1/nullnet on 10 3/sshd on 30,]".parse::<Processes>();
        assert_eq!(processes, Err("invalid digit found in string".to_string()));
    }
}
