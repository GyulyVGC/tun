use crate::commands::RtNetLinkHandle;
use crate::commands::ovs::OvsCommand;
use futures::StreamExt;
use ipnetwork::Ipv4Network;
use nullnet_liberror::{Error, ErrorHandler, Location, location};
use rtnetlink::packet_route::link::{LinkAttribute, LinkMessage};
use rtnetlink::{Handle, LinkUnspec, LinkVeth};
use std::net::IpAddr;

#[derive(Debug)]
pub(super) enum NetLinkCommand {
    HandleVethPairCreation(Ipv4Network, String, String),
    DeleteAllVeths,
    SetInterfacesUp(Vec<String>),
}

impl NetLinkCommand {
    pub(super) async fn execute(&self, rtnetlink_handle: &RtNetLinkHandle) {
        let handle = &rtnetlink_handle.handle;
        let init_t = std::time::Instant::now();
        match self {
            NetLinkCommand::HandleVethPairCreation(addr, veth_name, veth_peer_name) => {
                let _ = handle_veth_pair_creation(handle, *addr, veth_name, veth_peer_name).await;
            }
            NetLinkCommand::DeleteAllVeths => {
                delete_all_veths(handle).await;
            }
            NetLinkCommand::SetInterfacesUp(interfaces) => {
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
    net: Ipv4Network,
    veth_name: &String,
    veth_peer_name: &String,
) -> Result<(), Error> {
    let ip = net.ip();
    let prefix = net.prefix();

    // delete veth_name if it exists
    if let Some(Ok(link)) = handle
        .link()
        .get()
        .match_name(veth_name.clone())
        .execute()
        .next()
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
        .add(LinkVeth::new(veth_name, veth_peer_name).build())
        .execute()
        .await
        .handle_err(location!())?;

    // retrieve both ends of the veth pair
    let veth = get_link_by_name(handle, veth_name).await?;
    let veth_peer = get_link_by_name(handle, veth_peer_name).await?;

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

    Ok(())
}

async fn delete_all_veths(handle: &Handle) {
    let mut links = handle.link().get().execute();
    while let Some(link_res) = links.next().await {
        if let Ok(link) = link_res
            && link.attributes.iter().any(|attr| {
                if let LinkAttribute::IfName(name) = attr
                    && name.starts_with("veth")
                {
                    true
                } else {
                    false
                }
            })
        {
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
        .next()
        .await
        .ok_or(format!("Failed to find device {name}"))
        .handle_err(location!())?
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
