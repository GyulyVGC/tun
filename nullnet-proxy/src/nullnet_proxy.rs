use crate::service::{ServiceToml, ServicesToml};
use crate::{BrowserRequest, OvsVlan, Service};
use ipnetwork::Ipv4Network;
use std::collections::HashMap;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};

pub struct NullnetProxy {
    /// The available services and their host machine addresses
    services: HashMap<Service, SocketAddr>,
    /// Mapping of client IP + target service to upstream VLAN address
    connections: Arc<Mutex<HashMap<BrowserRequest, SocketAddr>>>,
    /// Last registered VLAN ID
    last_registered_vlan: Arc<Mutex<u16>>,
    /// UDP socket for sending VLAN setup requests
    udp_socket: Arc<UdpSocket>,
}

impl NullnetProxy {
    pub fn new() -> Self {
        let toml_str = fs::read_to_string("services.toml").expect("Failed to read services.toml");
        let services_toml =
            toml::from_str::<ServicesToml>(&toml_str).expect("Failed to parse services.toml");

        let services: HashMap<Service, SocketAddr> = services_toml
            .services
            .into_iter()
            .filter_map(ServiceToml::into_mapping)
            .collect();

        Self {
            services,
            connections: Arc::new(Mutex::new(HashMap::new())),
            last_registered_vlan: Arc::new(Mutex::new(100)),
            udp_socket: Arc::new(
                UdpSocket::bind("0.0.0.0:9997").expect("Failed to bind UDP socket"),
            ),
        }
    }

    pub fn get_or_add_upstream(&self, browser_req: BrowserRequest) -> Option<SocketAddr> {
        if let Some(upstream) = self.connections.lock().ok()?.get(&browser_req) {
            return Some(*upstream);
        }

        println!("Setting up new upstream for {browser_req}");

        let host = self.services.get(&browser_req.service)?;
        let host_ip = host.ip();
        let host_port = host.port();

        let vlan_id = {
            let mut last_id = self.last_registered_vlan.lock().ok()?;
            *last_id += 1;
            *last_id
        };
        let [a, b] = vlan_id.to_be_bytes();

        // create dedicated VLAN on this machine
        let port_ip = Ipv4Addr::new(10, a, b, 2);
        let ipv4_network = Ipv4Network::new(port_ip, 24).ok()?;
        self.send_vlan_setup_request(
            // this machine's IP
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 130)),
            vlan_id,
            vec![ipv4_network],
        )?;

        // create dedicated VLAN on target host and get newly created upstream address
        let port_ip = Ipv4Addr::new(10, a, b, 1);
        let ipv4_network = Ipv4Network::new(port_ip, 24).ok()?;
        self.send_vlan_setup_request(host_ip, vlan_id, vec![ipv4_network])?;

        let upstream = SocketAddr::new(IpAddr::V4(port_ip), host_port);
        self.connections.lock().ok()?.insert(browser_req, upstream);

        // wait a bit for VLAN setup to complete
        std::thread::sleep(std::time::Duration::from_secs(1));

        Some(upstream)
    }

    pub fn send_vlan_setup_request(
        &self,
        to: IpAddr,
        vlan_id: u16,
        vlan_ports: Vec<Ipv4Network>,
    ) -> Option<()> {
        let ovs_vlan = OvsVlan {
            id: vlan_id,
            ports: vlan_ports,
        };
        let request_body = toml::to_string(&ovs_vlan).ok()?;
        let to = SocketAddr::new(to, 9998);
        self.udp_socket.send_to(request_body.as_bytes(), to).ok()?;
        Some(())
    }
}
