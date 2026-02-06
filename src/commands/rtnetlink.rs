use crate::commands::RtNetLinkHandle;
use crate::commands::ovs::OvsCommand;
use futures::StreamExt;
use futures::stream::TryStreamExt;
use ipnetwork::Ipv4Network;
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use nullnet_liberror::{Error, ErrorHandler, Location, location};
use rtnetlink::packet_route::link::{LinkAttribute, LinkMessage};
use rtnetlink::{Handle, LinkUnspec, LinkVeth};
use std::net::IpAddr;

#[derive(Debug)]
pub(super) enum RtNetLinkCommand {
    HandleVethPairCreation(u16, Ipv4Network),
    DeleteAllVeths,
    SetInterfacesUp(Vec<String>),
}

impl RtNetLinkCommand {
    pub(super) async fn execute(&self, rtnetlink_handle: &RtNetLinkHandle) {
        let handle = &rtnetlink_handle.handle;
        let init_t = std::time::Instant::now();
        match self {
            RtNetLinkCommand::HandleVethPairCreation(vlan_id, addr) => {
                let _ = handle_veth_pair_creation(handle, *vlan_id, *addr).await;
            }
            RtNetLinkCommand::DeleteAllVeths => {
                delete_all_veths(handle).await;
            }
            RtNetLinkCommand::SetInterfacesUp(interfaces) => {
                set_interfaces_up(handle, interfaces).await;
            }
        }
        println!(
            "Executed command {:?} in {} ms",
            self,
            init_t.elapsed().as_millis()
        );
    }
}

async fn handle_veth_pair_creation(
    handle: &Handle,
    vlan_id: u16,
    net: Ipv4Network,
) -> Result<(), Error> {
    let ip = net.ip();
    let prefix = net.prefix();
    let veth_name = format!("veth{}", ip.to_bits());
    let veth_peer_name = format!("{veth_name}p");

    // delete veth_name if it exists
    if let Ok(Some(link)) = handle
        .link()
        .get()
        .match_name(veth_name.clone())
        .execute()
        .try_next()
        .await
    {
        handle
            .link()
            .del(link.header.index)
            .execute()
            .await
            .handle_err(location!())?;
    }

    // create veth pair veth_name <-> veth_peer_name
    handle
        .link()
        .add(LinkVeth::new(&veth_name, &veth_peer_name).build())
        .execute()
        .await
        .handle_err(location!())?;

    // retrieve both ends of the veth pair
    let veth = get_link_by_name(handle, &veth_name).await?;
    let veth_peer = get_link_by_name(handle, &veth_peer_name).await?;

    // set both ends of the veth pair up
    for link in [&veth, &veth_peer] {
        set_link_up(handle, link).await?;
    }

    handle
        .address()
        .add(veth.header.index, IpAddr::V4(ip), prefix)
        .execute()
        .await
        .handle_err(location!())?;

    // add the peer interface to the bridge as an access port
    OvsCommand::AddAccessPort(&veth_peer_name, vlan_id).execute();

    Ok(())
}

async fn delete_all_veths(handle: &Handle) {
    let links = handle
        .link()
        .get()
        .execute()
        .filter_map(|link_res| {
            link_res.map(|link| {
                link.attributes.iter().find_map(|attr| {
                    if let LinkAttribute::IfName(name) = attr {
                        if name.starts_with("veth") {
                            return Some(link);
                        }
                    }
                    None
                })
            })
        })
        .collect::<Vec<LinkMessage>>();

    for link in links {
        if let Ok(link) = link.await {
            let _ = handle.link().del(link.header.index).execute().await;
        }
    }
}

async fn set_interfaces_up(handle: &Handle, interfaces: &Vec<String>) {
    for dev in interfaces {
        if let Ok(link) = get_link_by_name(handle, dev).await {
            let _ = set_link_up(handle, &link).await;
        }
    }
}

// helpers -----------------------------------------------------------------------------------------

async fn get_link_by_name(handle: &Handle, name: &str) -> Result<LinkMessage, Error> {
    let link = handle
        .link()
        .get()
        .match_name(name.to_string())
        .execute()
        .try_next()
        .await
        .handle_err(location!())?
        .ok_or(format!("Failed to find device {name}"))
        .handle_err(location!())?;

    Ok(link)
}

async fn set_link_up(handle: &Handle, link: &LinkMessage) -> Result<(), Error> {
    let req = LinkUnspec::new_with_index(link.header.index).up().build();
    handle
        .link()
        .set(req)
        .execute()
        .await
        .handle_err(location!())?;

    Ok(())
}
