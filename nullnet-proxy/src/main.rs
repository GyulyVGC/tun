mod nullnet_proxy;
mod service;

use crate::nullnet_proxy::NullnetProxy;
use crate::service::Service;
use async_trait::async_trait;
use ipnetwork::Ipv4Network;
use pingora_core::server::Server;
use pingora_core::upstreams::peer::HttpPeer;
use pingora_core::{Error, ErrorType, Result};
use pingora_proxy::{ProxyHttp, Session};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::net::IpAddr;

const PROXY_PORT: u16 = 7777;

#[async_trait]
impl ProxyHttp for NullnetProxy {
    type CTX = ();
    fn new_ctx(&self) -> Self::CTX {}

    async fn upstream_peer(&self, session: &mut Session, _ctx: &mut ()) -> Result<Box<HttpPeer>> {
        let host_header = session
            .get_header("host")
            .ok_or_else(|| Error::explain(ErrorType::BindError, "No host header in request"))?;
        let host_str = host_header
            .to_str()
            .map_err(|_| Error::explain(ErrorType::BindError, "Invalid host header"))?;
        let url = host_str
            .strip_suffix(&format!(":{PROXY_PORT}"))
            .ok_or_else(|| {
                Error::explain(
                    ErrorType::BindError,
                    "Host header does not contain proxy port",
                )
            })?;
        let client_ip = session
            .client_addr()
            .ok_or_else(|| {
                Error::explain(ErrorType::BindError, "Client address not found in session")
            })?
            .as_inet()
            .ok_or_else(|| {
                Error::explain(
                    ErrorType::BindError,
                    "Client address is not an Inet address",
                )
            })?
            .ip();

        let service = Service(url.to_string());
        let client_req = BrowserRequest { client_ip, service };
        println!("{client_req}");
        let upstream = self
            .get_or_add_upstream(client_req)
            .ok_or_else(|| Error::explain(ErrorType::BindError, "Failed to retrieve upstream"))?;
        println!("upstream: {upstream}\n");

        let peer = Box::new(HttpPeer::new(upstream, false, String::new()));
        Ok(peer)
    }
}

fn main() {
    let proxy_address = format!("0.0.0.0:{PROXY_PORT}");
    println!("Running Nullnet proxy at {proxy_address}\n");

    // start proxy server
    let mut my_server = Server::new(None).expect("Failed to instantiate proxy server");
    my_server.bootstrap();

    let mut proxy =
        pingora_proxy::http_proxy_service(&my_server.configuration, NullnetProxy::new());
    proxy.add_tcp(&proxy_address);

    my_server.add_service(proxy);
    my_server.run_forever();
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct BrowserRequest {
    pub(crate) client_ip: IpAddr,
    pub(crate) service: Service,
}

impl Display for BrowserRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} -> {}", self.client_ip, self.service.0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct OvsVlan {
    pub id: u16,
    pub ports: Vec<Ipv4Network>,
}

#[cfg(test)]
mod tests {

    use crate::OvsVlan;
    use ipnetwork::Ipv4Network;
    use serde_test::{Configure, Token, assert_tokens};
    use std::net::Ipv4Addr;

    fn vlan_for_tests() -> OvsVlan {
        OvsVlan {
            id: 10,
            ports: vec![
                Ipv4Network::new(Ipv4Addr::new(8, 8, 8, 8), 24).unwrap(),
                Ipv4Network::new(Ipv4Addr::new(16, 16, 16, 16), 8).unwrap(),
            ],
        }
    }

    #[test]
    fn test_serialize_and_deserialize_vlan() {
        let vlan_setup_request = vlan_for_tests();

        assert_tokens(
            &vlan_setup_request.readable(),
            &[
                Token::Struct {
                    name: "OvsVlan",
                    len: 2,
                },
                Token::Str("id"),
                Token::U16(10),
                Token::Str("ports"),
                Token::Seq { len: Some(2) },
                Token::Str("8.8.8.8/24"),
                Token::Str("16.16.16.16/8"),
                Token::SeqEnd,
                Token::StructEnd,
            ],
        );
    }

    #[test]
    fn test_toml_string_vlan() {
        let vlan_setup_request = vlan_for_tests();

        assert_eq!(
            toml::to_string(&vlan_setup_request).unwrap(),
            "id = 10\n\
             ports = [\"8.8.8.8/24\", \"16.16.16.16/8\"]\n"
        );
    }
}
