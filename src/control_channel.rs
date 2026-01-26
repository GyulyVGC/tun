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
        let Ok(target_ip) = message.target_ip.parse::<Ipv4Addr>() else {
            continue;
        };
        let Ok(veth_ip) = message.veth_ip.parse::<Ipv4Addr>() else {
            continue;
        };
        let Ok(vlan_id) = u16::try_from(message.vlan_id) else {
            continue;
        };

        let veth_interface = VethInterface::new(veth_ip, vlan_id);

        if target_ip == local_ethernet.ip {
            // setup VLAN on this machine
            println!("setting up veth {veth_ip} on VLAN {vlan_id}");
            veth_interface.activate();

            // add host mapping if needed
            if let Some(host_mapping) = message.host_mapping {
                let _ = add_host_mapping(&host_mapping);
            }
        } else {
            // register peer
            println!("registering peer {veth_ip} on VLAN {vlan_id} for target IP {target_ip}");
            peers
                .write()
                .await
                .insert(veth_interface.get_veth_key(), target_ip);
        }

        // acknowledge message
        let _ = outbound.send(Empty {}).await;
    }

    Ok(())
}

fn add_host_mapping(hm: &HostMapping) -> Result<(), Error> {
    let path = "/etc/hosts";
    let entry = format!("{} {}", hm.ip, hm.name);

    println!("Adding host mapping: {entry}");

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

// TODO
// - inactive peer removal
// - heartbeat messages
