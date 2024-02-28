use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use std::fmt::{Display, Formatter};
use tokio_rusqlite::types::ToSqlOutput;
use tokio_rusqlite::ToSql;

#[derive(Serialize, Deserialize, PartialEq, Debug, Default, Clone)]
pub struct ListenerNames {
    #[serde(rename = "process_names")]
    pub(crate) names: BTreeSet<String>,
}

impl ListenerNames {
    pub fn from_set(set: HashSet<String>) -> Self {
        Self {
            names: set.into_iter().collect(),
        }
    }
}

impl Display for ListenerNames {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut str = format!("{:?}", self.names);
        str = str.replace('"', "");
        str = str.replace('{', "[");
        str = str.replace('}', "]");
        write!(f, "{str}")
    }
}

impl ToSql for ListenerNames {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.to_string().into())
    }
}

#[cfg(test)]
mod tests {
    use crate::peers::listener_names::ListenerNames;
    use rusqlite::ToSql;
    use std::collections::BTreeSet;
    use tokio_rusqlite::types::ToSqlOutput;
    use tokio_rusqlite::types::Value::Text;

    #[test]
    fn test_listener_processes_to_sql() {
        let listener_processes = ListenerNames {
            names: BTreeSet::from([
                "nullnet".to_string(),
                "nullnetd".to_string(),
                "tun".to_string(),
            ]),
        };
        assert_eq!(
            ListenerNames::to_sql(&listener_processes),
            Ok(ToSqlOutput::Owned(Text(
                "[nullnet, nullnetd, tun]".to_string()
            )))
        );
    }
}
