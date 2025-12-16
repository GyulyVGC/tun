use crate::TAP_NAME;
use ipnetwork::{Ipv4Network};
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use nullnet_liberror::{ErrorHandler, Location, location};
use std::process::Command;

enum OvsCommand<'a> {
    DeleteInterface(&'a str),
    DeleteBridge,
    AddBridge,
    SetInterfaceUp(&'a str),
    DeleteFlows,
    AddFlow,
    AddTrunkPort,
    CreateVethPair(&'a str, &'a str),
    AssignIpToInterface(&'a str, Ipv4Network),
    AddAccessPort(&'a str, u16),
}

impl OvsCommand<'_> {
    fn execute(&self) {
        let _ = Command::new(self.program())
            .args(self.args())
            .spawn()
            .map(|mut c| c.wait())
            .handle_err(location!());
    }

    fn program(&self) -> &str {
        match self {
            OvsCommand::AddBridge
            | OvsCommand::DeleteBridge
            | OvsCommand::AddAccessPort(_, _)
            | OvsCommand::AddTrunkPort => "ovs-vsctl",
            OvsCommand::SetInterfaceUp(_)
            | OvsCommand::DeleteInterface(_)
            | OvsCommand::AssignIpToInterface(_, _)
            | OvsCommand::CreateVethPair(_, _) => "ip",
            OvsCommand::DeleteFlows | OvsCommand::AddFlow => "ovs-ofctl",
        }
    }

    fn args(&self) -> Vec<String> {
        match self {
            OvsCommand::AddBridge => ["add-br", "br0"].iter().map(ToString::to_string).collect(),
            OvsCommand::DeleteBridge => ["del-br", "br0"].iter().map(ToString::to_string).collect(),
            OvsCommand::DeleteFlows => ["del-flows", "br0"]
                .iter()
                .map(ToString::to_string)
                .collect(),
            OvsCommand::AddFlow => ["add-flow", "br0", "priority=0,actions=normal"]
                .iter()
                .map(ToString::to_string)
                .collect(),
            OvsCommand::AddTrunkPort => ["add-port", "br0", TAP_NAME]
                .iter()
                .map(ToString::to_string)
                .collect(),
            OvsCommand::SetInterfaceUp(dev) => ["link", "set", dev, "up"]
                .iter()
                .map(ToString::to_string)
                .collect(),
            OvsCommand::DeleteInterface(dev) => ["link", "del", dev]
                .iter()
                .map(ToString::to_string)
                .collect(),
            OvsCommand::CreateVethPair(veth, vethp) => {
                ["link", "add", veth, "type", "veth", "peer", "name", vethp]
                    .iter()
                    .map(ToString::to_string)
                    .collect()
            }
            OvsCommand::AssignIpToInterface(dev, net) => {
                ["addr", "add", &net.to_string(), "dev", dev]
                    .iter()
                    .map(ToString::to_string)
                    .collect()
            }
            OvsCommand::AddAccessPort(dev, vlan) => {
                ["add-port", "br0", dev, &format!("tag={vlan}")]
                    .iter()
                    .map(ToString::to_string)
                    .collect()
            }
        }
    }
}

pub(super) fn setup_br0() {
    // clean up existing veth interfaces
    delete_all_veths();

    // delete existing bridge if any
    OvsCommand::DeleteBridge.execute();

    // create the bridge
    OvsCommand::AddBridge.execute();

    // set the bridge up and ovs-system up
    for dev in ["br0", "ovs-system"] {
        OvsCommand::SetInterfaceUp(dev).execute();
    }

    // delete existing OpenFlow rules
    OvsCommand::DeleteFlows.execute();

    // use the built-in switching logic
    OvsCommand::AddFlow.execute();

    // add our TAP to the bridge as a trunk port
    OvsCommand::AddTrunkPort.execute();
}

pub(super) fn configure_access_port(vlan_id: u16, net: Ipv4Network) {
    let ip = net.ip();
    let veth_name = format!("veth{}", ip.to_bits());
    let veth_peer_name = format!("{veth_name}p");

    // delete existing veth pair if any
    OvsCommand::DeleteInterface(&veth_name).execute();

    // create veth pair
    OvsCommand::CreateVethPair(&veth_name, &veth_peer_name).execute();

    // set the veth interfaces up
    for dev in [&veth_name, &veth_peer_name] {
        OvsCommand::SetInterfaceUp(dev).execute();
    }

    // assign IP address to veth interface
    OvsCommand::AssignIpToInterface(&veth_name, net).execute();

    // add the peer interface to the bridge as an access port
    OvsCommand::AddAccessPort(&veth_peer_name, vlan_id).execute();
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
