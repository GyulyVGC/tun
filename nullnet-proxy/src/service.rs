use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct Service(pub String);

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub(crate) struct ServicesToml {
    pub(crate) services: Vec<ServiceToml>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub(crate) struct ServiceToml {
    name: String,
    host: String,
    port: u16,
}

impl ServiceToml {
    pub(crate) fn into_mapping(self) -> Option<(Service, SocketAddr)> {
        let service = Service(self.name);
        let host_ip = self.host.parse().ok()?;
        let host_addr = SocketAddr::new(host_ip, self.port);
        Some((service, host_addr))
    }
}

#[cfg(test)]
mod tests {
    use crate::service::{ServiceToml, ServicesToml};
    use serde_test::{Configure, Token, assert_tokens};

    fn services_for_tests() -> ServicesToml {
        ServicesToml {
            services: vec![
                ServiceToml {
                    name: "color.com".to_string(),
                    host: "192.168.1.104".to_string(),
                    port: 3001,
                },
                ServiceToml {
                    name: "directory.com".to_string(),
                    host: "192.168.1.104".to_string(),
                    port: 8080,
                },
            ],
        }
    }

    #[test]
    fn test_serialize_and_deserialize_services() {
        let services = services_for_tests();

        assert_tokens(
            &services.readable(),
            &[
                Token::Struct {
                    name: "ServicesToml",
                    len: 1,
                },
                Token::Str("services"),
                Token::Seq { len: Some(2) },
                Token::Struct {
                    name: "ServiceToml",
                    len: 3,
                },
                Token::Str("name"),
                Token::Str("color.com"),
                Token::Str("host"),
                Token::Str("192.168.1.104"),
                Token::Str("port"),
                Token::U16(3001),
                Token::StructEnd,
                Token::Struct {
                    name: "ServiceToml",
                    len: 3,
                },
                Token::Str("name"),
                Token::Str("directory.com"),
                Token::Str("host"),
                Token::Str("192.168.1.104"),
                Token::Str("port"),
                Token::U16(8080),
                Token::StructEnd,
                Token::SeqEnd,
                Token::StructEnd,
            ],
        );
    }

    #[test]
    fn test_toml_string_services() {
        let services = services_for_tests();

        assert_eq!(
            toml::to_string(&services).unwrap(),
            "[[services]]\n\
             name = \"color.com\"\n\
             host = \"192.168.1.104\"\n\
             port = 3001\n\n\
             [[services]]\n\
             name = \"directory.com\"\n\
             host = \"192.168.1.104\"\n\
             port = 8080\n"
        );
    }
}
