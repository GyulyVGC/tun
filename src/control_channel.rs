use crate::commands::{RtNetLinkHandle, configure_access_port, remove_vlan};
use crate::peers::peer::{Peers, VethKey};
use ipnetwork::Ipv4Network;
use nullnet_grpc_lib::NullnetGrpcInterface;
use nullnet_grpc_lib::nullnet_grpc::{
    HostMapping, MsgId, VlanSetup, VlanTeardown, VxlanSetup, VxlanTeardown, net_message,
};
use nullnet_liberror::{Error, ErrorHandler, Location, location};
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::{RwLock, mpsc};

pub(crate) async fn control_channel(
    server: NullnetGrpcInterface,
    peers: Arc<RwLock<Peers>>,
    rtnetlink_handle: RtNetLinkHandle,
) -> Result<(), Error> {
    let (outbound, grpc_rx) = mpsc::channel(64);
    let mut inbound = server
        .control_channel(grpc_rx)
        .await
        .handle_err(location!())?;

    while let Ok(Some(message)) = inbound.message().await {
        let rtnetlink_handle = rtnetlink_handle.clone();
        let peers = peers.clone();
        let outbound = outbound.clone();
        match message.message {
            Some(net_message::Message::VlanSetup(vlan_setup)) => {
                tokio::spawn(async move {
                    let _ = handle_vlan_setup(vlan_setup, rtnetlink_handle, peers, outbound).await;
                });
            }
            Some(net_message::Message::VlanTeardown(vlan_teardown)) => {
                tokio::spawn(async move {
                    let _ = handle_vlan_teardown(vlan_teardown, rtnetlink_handle, peers).await;
                });
            }
            Some(net_message::Message::VxlanSetup(vxlan_setup)) => {
                tokio::spawn(async move {
                    let _ = handle_vxlan_setup(vxlan_setup, outbound).await;
                });
            }
            Some(net_message::Message::VxlanTeardown(vxlan_teardown)) => {
                tokio::spawn(async move {
                    handle_vxlan_teardown(vxlan_teardown);
                });
            }
            None => {}
        }
    }

    Ok(())
}

async fn handle_vlan_setup(
    message: VlanSetup,
    rtnetlink_handle: RtNetLinkHandle,
    peers: Arc<RwLock<Peers>>,
    outbound: Sender<MsgId>,
) -> Result<(), Error> {
    let msg_id = &message
        .msg_id
        .ok_or("Missing message ID in VXLAN setup message")
        .handle_err(location!())?;
    let local_veth = message
        .local_veth
        .parse::<Ipv4Addr>()
        .handle_err(location!())?;
    let remote_ip = message
        .remote_ip
        .parse::<Ipv4Addr>()
        .handle_err(location!())?;
    let remote_veth = message
        .remote_veth
        .parse::<Ipv4Addr>()
        .handle_err(location!())?;
    let vlan_id = u16::try_from(message.vlan_id).handle_err(location!())?;

    // setup VLAN on this machine
    let init_t = std::time::Instant::now();
    configure_access_port(
        &rtnetlink_handle,
        vlan_id,
        Ipv4Network::new(local_veth, 24).unwrap(),
    )
    .await;
    println!(
        "veth {local_veth} setup completed in {} ms",
        init_t.elapsed().as_millis()
    );

    // register peer
    peers
        .write()
        .await
        .insert(VethKey::new(remote_veth, vlan_id), remote_ip);

    // add host mapping if needed
    if let Some(host_mapping) = &message.host_mapping {
        let _ = add_host_mapping(host_mapping, None);
    }

    // acknowledge message
    let _ = outbound.send(msg_id.clone()).await;

    Ok(())
}

async fn handle_vlan_teardown(
    message: VlanTeardown,
    rtnetlink_handle: RtNetLinkHandle,
    peers: Arc<RwLock<Peers>>,
) -> Result<(), Error> {
    let vlan_id = u16::try_from(message.vlan_id).handle_err(location!())?;

    // teardown VLAN on this machine
    let init_t = std::time::Instant::now();

    remove_vlan(&rtnetlink_handle, vlan_id).await;

    println!(
        "VLAN teardown completed in {} ms",
        init_t.elapsed().as_millis()
    );

    // remove peer
    peers.write().await.remove(vlan_id);

    Ok(())
}

async fn handle_vxlan_setup(message: VxlanSetup, outbound: Sender<MsgId>) -> Result<(), Error> {
    let msg_id = &message
        .msg_id
        .ok_or("Missing message ID in VXLAN setup message")
        .handle_err(location!())?;
    let vxlan_id = message.vxlan_id;
    let ns_name = message.ns_name;
    let ns_net = message
        .ns_net
        .parse::<Ipv4Network>()
        .handle_err(location!())?;
    let br_name = message.br_name;
    let br_net = message
        .br_net
        .parse::<Ipv4Network>()
        .handle_err(location!())?;
    let local_ip = message
        .local_ip
        .parse::<Ipv4Addr>()
        .handle_err(location!())?;
    let remote_ip = message
        .remote_ip
        .parse::<Ipv4Addr>()
        .handle_err(location!())?;

    // setup VXLAN on this machine (optionally attaching a Docker container)
    let init_t = std::time::Instant::now();
    let mut cmd = std::process::Command::new("./vxlan_scripts/vxlan-setup.sh");
    cmd.arg(vxlan_id.to_string())
        .arg(&ns_name)
        .arg(ns_net.to_string())
        .arg(br_name)
        .arg(br_net.to_string())
        .arg(local_ip.to_string())
        .arg(remote_ip.to_string());
    if let Some(container) = &message.docker_container {
        cmd.arg(container);
    }
    let _ = cmd.spawn().map(|mut c| c.wait()).handle_err(location!());
    println!(
        "VXLAN {vxlan_id} setup completed in {} ms (docker: {})",
        init_t.elapsed().as_millis(),
        message.docker_container.as_deref().unwrap_or("none"),
    );

    // add host mapping if needed
    if let Some(host_mapping) = &message.host_mapping {
        let _ = add_host_mapping(host_mapping, message.docker_container.as_deref());
    }

    // acknowledge message
    let _ = outbound.send(msg_id.clone()).await;

    Ok(())
}

fn handle_vxlan_teardown(message: VxlanTeardown) {
    // teardown VXLAN on this machine
    let init_t = std::time::Instant::now();

    let vxlan_id = message.vxlan_id;
    let ns_name = format!("ns_{vxlan_id}");
    let br_name = format!("br_{vxlan_id}");

    let mut cmd = std::process::Command::new("./vxlan_scripts/vxlan-teardown.sh");
    cmd.arg(&ns_name).arg(&br_name);
    if let Some(container) = &message.docker_container {
        cmd.arg(container);
    }
    let _ = cmd.spawn().map(|mut c| c.wait()).handle_err(location!());

    println!(
        "VXLAN teardown completed in {} ms",
        init_t.elapsed().as_millis()
    );
}

fn add_host_mapping(hm: &HostMapping, docker_container: Option<&str>) -> Result<(), Error> {
    let path = "/etc/hosts";
    let entry = format!("{} {}", hm.ip, hm.name);

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

    // copy /etc/hosts content into the container
    if let Some(container) = docker_container {
        let content = std::fs::read_to_string(path).handle_err(location!())?;
        let _ = std::process::Command::new("docker")
            .args([
                "exec",
                container,
                "sh",
                "-c",
                &format!("echo '{content}' > {path}"),
            ])
            .spawn()
            .map(|mut c| c.wait())
            .handle_err(location!());
    }

    Ok(())
}
