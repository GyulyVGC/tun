use crate::ovs::veth_interface::VethInterface;
use crate::peers::ethernet_addr::EthernetAddr;
use crate::peers::peer::Peers;
use nullnet_grpc_lib::NullnetGrpcInterface;
use nullnet_grpc_lib::nullnet_grpc::{Empty, HostMapping};
use nullnet_liberror::{Error, ErrorHandler, Location, location};
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};

pub(crate) async fn control_channel(
    server: NullnetGrpcInterface,
    local_ethernet: EthernetAddr,
    peers: Arc<RwLock<Peers>>,
) -> Result<(), Error> {
    let (outbound, grpc_rx) = mpsc::channel(64);
    let mut inbound = server
        .control_channel(grpc_rx)
        .await
        .handle_err(location!())?;

    while let Ok(Some(message)) = inbound.message().await {
        let Ok(client_eth) = message.client_eth.parse::<Ipv4Addr>() else {
            continue;
        };
        let Ok(client_veth) = message.client_veth.parse::<Ipv4Addr>() else {
            continue;
        };
        let Ok(server_eth) = message.server_eth.parse::<Ipv4Addr>() else {
            continue;
        };
        let Ok(server_veth) = message.server_veth.parse::<Ipv4Addr>() else {
            continue;
        };
        let Ok(vlan_id) = u16::try_from(message.vlan_id) else {
            continue;
        };

        let local_ip = local_ethernet.ip;

        let client_veth_interface = VethInterface::new(client_veth, vlan_id);
        let server_veth_interface = VethInterface::new(server_veth, vlan_id);

        if client_eth == local_ip {
            // setup VLAN on this machine
            let init_t = std::time::Instant::now();
            client_veth_interface.activate();
            println!(
                "veth {client_veth} setup completed in {} ms",
                init_t.elapsed().as_millis()
            );

            // register peer
            // println!("registering peer {server_veth} on VLAN {vlan_id} for target IP {server_eth}");
            peers
                .write()
                .await
                .insert(server_veth_interface.get_veth_key(), server_eth);

            // add host mapping if needed
            if let Some(host_mapping) = &message.host_mapping {
                let init_t = std::time::Instant::now();
                let _ = add_host_mapping(host_mapping);
                println!(
                    "host mapping {} -> {} added in {} ms",
                    host_mapping.name,
                    host_mapping.ip,
                    init_t.elapsed().as_millis()
                );
            }
        }

        if server_eth == local_ip {
            // setup VLAN on this machine
            let init_t = std::time::Instant::now();
            server_veth_interface.activate();
            println!(
                "veth {server_veth} setup completed in {} ms",
                init_t.elapsed().as_millis()
            );

            // register peer
            // println!("registering peer {client_veth} on VLAN {vlan_id} for target IP {client_eth}");
            peers
                .write()
                .await
                .insert(client_veth_interface.get_veth_key(), client_eth);
        }

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
