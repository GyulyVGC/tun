use crate::commands::{RtNetLinkHandle, configure_access_port};
use crate::peers::ethernet_addr::EthernetAddr;
use crate::peers::peer::{Peers, VethKey};
use ipnetwork::Ipv4Network;
use nullnet_grpc_lib::NullnetGrpcInterface;
use nullnet_grpc_lib::nullnet_grpc::{Empty, HostMapping};
use nullnet_liberror::{Error, ErrorHandler, Location, location};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio::task::JoinSet;

#[derive(Copy, Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
struct VethInterface {
    ip: Ipv4Network,
    vlan_id: u16,
}

impl VethInterface {
    fn new(ip: Ipv4Addr, vlan_id: u16) -> Result<Self, Error> {
        let ip = Ipv4Network::new(ip, 24).handle_err(location!())?;
        Ok(Self { ip, vlan_id })
    }

    async fn activate(&self, rtnetlink_handle: &RtNetLinkHandle) {
        configure_access_port(rtnetlink_handle, self.vlan_id, self.ip).await;
    }

    fn get_veth_key(self) -> VethKey {
        VethKey::new(self.ip.ip(), self.vlan_id)
    }
}

pub(crate) async fn control_channel(
    server: NullnetGrpcInterface,
    local_ethernet: EthernetAddr,
    peers: Arc<RwLock<Peers>>,
    rtnetlink_handle: RtNetLinkHandle,
) -> Result<(), Error> {
    let (outbound, grpc_rx) = mpsc::channel(64);
    let mut inbound = server
        .control_channel(grpc_rx)
        .await
        .handle_err(location!())?;

    while let Ok(Some(message)) = inbound.message().await {
        let Ok(client_ethernet) = message.client_ethernet.parse::<Ipv4Addr>() else {
            continue;
        };
        let Ok(client_veth) = message.client_veth.parse::<Ipv4Addr>() else {
            continue;
        };
        let Ok(server_ethernet) = message.server_ethernet.parse::<Ipv4Addr>() else {
            continue;
        };
        let Ok(server_veth) = message.server_veth.parse::<Ipv4Addr>() else {
            continue;
        };
        let Ok(vlan_id) = u16::try_from(message.vlan_id) else {
            continue;
        };

        let local_ip = local_ethernet.ip;

        let client_veth_interface = VethInterface::new(client_veth, vlan_id)?;
        let server_veth_interface = VethInterface::new(server_veth, vlan_id)?;

        let mut join_set = JoinSet::new();
        if client_ethernet == local_ip {
            let rtnetlink_handle = rtnetlink_handle.clone();
            let peers = peers.clone();
            join_set.spawn(async move {
                // setup VLAN on this machine
                let init_t = std::time::Instant::now();
                client_veth_interface.activate(&rtnetlink_handle).await;
                println!(
                    "veth {client_veth} setup completed in {} ms",
                    init_t.elapsed().as_millis()
                );

                // register peer
                peers
                    .write()
                    .await
                    .insert(server_veth_interface.get_veth_key(), server_ethernet);

                // add host mapping if needed
                if let Some(host_mapping) = &message.host_mapping {
                    let _ = add_host_mapping(host_mapping);
                }
            });
        }

        if server_ethernet == local_ip {
            let rtnetlink_handle = rtnetlink_handle.clone();
            let peers = peers.clone();
            join_set.spawn(async move {
                // setup VLAN on this machine
                let init_t = std::time::Instant::now();
                server_veth_interface.activate(&rtnetlink_handle).await;
                println!(
                    "veth {server_veth} setup completed in {} ms",
                    init_t.elapsed().as_millis()
                );

                // register peer
                peers
                    .write()
                    .await
                    .insert(client_veth_interface.get_veth_key(), client_ethernet);
            });
        }

        while join_set.join_next().await.is_some() {}

        // acknowledge message
        let _ = outbound.send(Empty {}).await;
    }

    Ok(())
}

fn add_host_mapping(hm: &HostMapping) -> Result<(), Error> {
    let path = "/etc/hosts";
    let entry = format!("{} {}", hm.ip, hm.name);

    // println!("Adding host mapping: {entry}");

    // parse each line IP and name: if name exists replace the line, else append
    let content = std::fs::read_to_string(path).handle_err(location!())?;
    let mut lines: Vec<String> = content.lines().map(ToString::to_string).collect();
    let mut found = false;
    for line in &mut lines {
        if line.contains(&hm.name) {
            line.clone_from(&entry);
            found = true;
        }
    }
    if !found {
        lines.push(entry);
    }
    std::fs::write(path, lines.join("\n") + "\n").handle_err(location!())?;
    Ok(())
}

// TODO: inactive peer removal
