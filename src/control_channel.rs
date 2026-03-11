use crate::commands::{RtNetLinkHandle, configure_access_port};
use crate::peers::peer::{Peers, VethKey};
use ipnetwork::Ipv4Network;
use nullnet_grpc_lib::NullnetGrpcInterface;
use nullnet_grpc_lib::nullnet_grpc::{
    HostMapping, MsgId, VlanSetup, VlanTeardown, VxlanSetup, VxlanTeardown, net_message,
};
use nullnet_liberror::{Error, ErrorHandler, Location, location};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::{RwLock, mpsc};

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
struct VxlanInterface {
    vxlan_id: u32,
    ns_name: String,
    ns_net: Ipv4Network,
    br_name: String,
    br_net: Ipv4Network,
    local_ip: Ipv4Addr,
    remote_ip: Ipv4Addr,
}

impl VxlanInterface {
    // fn new(ip: Ipv4Addr, vlan_id: u16) -> Result<Self, Error> {
    //     let ip = Ipv4Network::new(ip, 24).handle_err(location!())?;
    //     Ok(Self { ip, vlan_id })
    // }

    // async fn activate(&self, rtnetlink_handle: &RtNetLinkHandle) {
    //     configure_access_port(rtnetlink_handle, self.vlan_id, self.ip).await;
    // }

    // fn get_veth_key(self) -> VethKey {
    //     VethKey::new(self.ip.ip(), self.vlan_id)
    // }

    fn new(vxlan_setup: &VxlanSetup) -> Result<Self, Error> {
        let ns_net = vxlan_setup
            .ns_net
            .parse::<Ipv4Network>()
            .handle_err(location!())?;
        let br_net = vxlan_setup
            .br_net
            .parse::<Ipv4Network>()
            .handle_err(location!())?;
        let local_ip = vxlan_setup
            .local_ip
            .parse::<Ipv4Addr>()
            .handle_err(location!())?;
        let remote_ip = vxlan_setup
            .remote_ip
            .parse::<Ipv4Addr>()
            .handle_err(location!())?;
        Ok(Self {
            vxlan_id: vxlan_setup.vxlan_id,
            ns_name: vxlan_setup.ns_name.clone(),
            ns_net,
            br_name: vxlan_setup.br_name.clone(),
            br_net,
            local_ip,
            remote_ip,
        })
    }

    fn activate(&self) {
        let _ = std::process::Command::new("./vxlan-setup.sh")
            .arg(self.vxlan_id.to_string())
            .arg(&self.ns_name)
            .arg(self.ns_net.to_string())
            .arg(&self.br_name)
            .arg(self.br_net.to_string())
            .arg(self.local_ip.to_string())
            .arg(self.remote_ip.to_string())
            .spawn()
            .map(|mut c| c.wait())
            .handle_err(location!());
    }
}

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
                    handle_vlan_setup(vlan_setup, rtnetlink_handle, peers, outbound).await;
                });
            }
            Some(net_message::Message::VlanTeardown(vlan_teardown)) => {
                tokio::spawn(async move {
                    handle_vlan_teardown(vlan_teardown);
                });
            }
            Some(net_message::Message::VxlanSetup(vxlan_setup)) => {
                tokio::spawn(async move {
                    handle_vxlan_setup(vxlan_setup, outbound).await;
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
) {
    let Some(msg_id) = message.msg_id else {
        return;
    };
    let Ok(local_ip) = message.local_ip.parse::<Ipv4Addr>() else {
        return;
    };
    let Ok(local_veth) = message.local_veth.parse::<Ipv4Addr>() else {
        return;
    };
    let Ok(remote_ip) = message.remote_ip.parse::<Ipv4Addr>() else {
        return;
    };
    let Ok(remote_veth) = message.remote_veth.parse::<Ipv4Addr>() else {
        return;
    };
    let Ok(vlan_id) = u16::try_from(message.vlan_id) else {
        return;
    };

    // setup VLAN on this machine
    let init_t = std::time::Instant::now();
    configure_access_port(
        &rtnetlink_handle,
        vlan_id,
        Ipv4Network::new(local_ip, 24).unwrap(),
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
        let _ = add_host_mapping(host_mapping);
    }

    // acknowledge message
    let _ = outbound.send(msg_id).await;
}

fn handle_vlan_teardown(_message: VlanTeardown) {
    // TODO: teardown VLAN on this machine
}

async fn handle_vxlan_setup(message: VxlanSetup, outbound: Sender<MsgId>) {
    let Some(msg_id) = &message.msg_id else {
        return;
    };
    let Ok(vxlan_interface) = VxlanInterface::new(&message) else {
        return;
    };

    // let rtnetlink_handle = rtnetlink_handle.clone();
    // setup VLAN on this machine
    let init_t = std::time::Instant::now();
    vxlan_interface.activate();
    println!(
        "VXLAN {} setup completed in {} ms",
        vxlan_interface.vxlan_id,
        init_t.elapsed().as_millis()
    );

    // register peer
    // peers
    //     .write()
    //     .await
    //     .insert(server_veth_interface.get_veth_key(), server_ethernet);

    // add host mapping if needed
    if let Some(host_mapping) = &message.host_mapping {
        let _ = add_host_mapping(host_mapping);
    }

    // acknowledge message
    let _ = outbound.send(msg_id.clone()).await;
}

fn handle_vxlan_teardown(message: VxlanTeardown) {
    // teardown VXLAN on this machine
    let init_t = std::time::Instant::now();

    let _ = std::process::Command::new("./vxlan-teardown.sh")
        .arg(message.ns_name)
        .arg(message.br_name)
        .spawn()
        .map(|mut c| c.wait())
        .handle_err(location!());

    println!(
        "VXLAN teardown completed in {} ms",
        init_t.elapsed().as_millis()
    );
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
