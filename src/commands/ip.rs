use crate::TAP_NAME;
use crate::commands::ovs::OvsCommand;
use futures::stream::TryStreamExt;
use ipnetwork::Ipv4Network;
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use nullnet_liberror::{Error, ErrorHandler, Location, location};
use rtnetlink::{LinkMessageBuilder, LinkUnspec, LinkVeth};
use std::net::IpAddr;
use std::process::Command;

#[derive(Debug)]
pub(super) enum IpCommand<'a> {
    HandleVethPairCreation(u16, Ipv4Network),
    DeleteAllVeths,
    SetInterfacesUp(Vec<String>),
}

impl IpCommand<'_> {
    pub(super) async fn execute(&self) {
        let init_t = std::time::Instant::now();
        match self {
            IpCommand::HandleVethPairCreation(vlan_id, addr) => {
                let _ = handle_veth_pair_creation(*vlan_id, *addr).await;
            }
            IpCommand::DeleteAllVeths => {
                delete_all_veths();
            }
            IpCommand::SetInterfacesUp(interfaces) => {
                set_interfaces_up(interfaces);
            }
        }
        println!(
            "Executed command {:?} in {} ms",
            self,
            init_t.elapsed().as_millis()
        );
    }
}

async fn handle_veth_pair_creation(vlan_id: u16, net: Ipv4Network) -> Result<(), Error> {
    let ip = net.ip();
    let prefix = net.prefix();
    let veth_name = format!("veth{}", ip.to_bits());
    let veth_peer_name = format!("{veth_name}p");

    let (connection, mut handle, _) = rtnetlink::new_connection().handle_err(location!())?;
    tokio::spawn(connection);

    // delete veth_name if it exists
    if let Some(link) = handle
        .link()
        .get()
        .match_name(veth_name.clone())
        .execute()
        .try_next()
        .await
        .handle_err(location!())?
    {
        handle
            .link()
            .del(link.header.index)
            .execute()
            .await
            .handle_err(location!())?;
    }

    // create veth pair veth_name <-> veth_peer_name and set it up
    handle
        .link()
        .add(LinkVeth::new(&veth_name, &veth_peer_name).up().build())
        .execute()
        .await
        .handle_err(location!())?;

    // assign IP to veth_name
    let link = handle
        .link()
        .get()
        .match_name(veth_name.clone())
        .execute()
        .try_next()
        .await
        .handle_err(location!())?
        .ok_or(Err("Failed to find veth interface after creation"))
        .handle_err(location!())?;
    handle
        .address()
        .add(link.header.index, IpAddr::V4(ip), prefix)
        .execute()
        .await
        .handle_err(location!())?;

    // add the peer interface to the bridge as an access port
    OvsCommand::AddAccessPort(&veth_peer_name, vlan_id).execute();

    Ok(())
}

fn delete_all_veths() {
    if let Ok(devices) = NetworkInterface::show() {
        for device in devices {
            let name = &device.name;
            if name.starts_with("veth") {
                OvsCommand::DeleteInterface(name).execute();
            }
        }
    }
}

fn set_interfaces_up(interfaces: &Vec<String>) {
    for dev in interfaces {
        OvsCommand::SetInterfaceUp(dev).execute();
    }
}
