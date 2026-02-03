use crate::TAP_NAME;
use ipnetwork::Ipv4Network;
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use nullnet_liberror::{ErrorHandler, Location, location};
use std::process::Command;

#[derive(Debug)]
pub(super) enum OvsCommand<'a> {
    DeleteInterface(&'a str),
    DeleteBridge,
    AddBridge,
    SetInterfaceUp(&'a str),
    DeleteFlows,
    AddFlow,
    AddTrunkPort,
    AddAccessPort(&'a str, u16),
}

impl OvsCommand<'_> {
    pub(super) fn execute(&self) {
        let init_t = std::time::Instant::now();
        let _ = Command::new(self.program())
            .args(self.args())
            .spawn()
            .map(|mut c| c.wait())
            .handle_err(location!());
        println!(
            "Executed command {:?} in {} ms",
            self,
            init_t.elapsed().as_millis()
        );
    }

    fn program(&self) -> &str {
        match self {
            OvsCommand::AddBridge
            | OvsCommand::DeleteBridge
            | OvsCommand::AddAccessPort(_, _)
            | OvsCommand::AddTrunkPort => "ovs-vsctl",
            OvsCommand::SetInterfaceUp(_) | OvsCommand::DeleteInterface(_) => "ip",
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
            OvsCommand::AddAccessPort(dev, vlan) => {
                ["add-port", "br0", dev, &format!("tag={vlan}")]
                    .iter()
                    .map(ToString::to_string)
                    .collect()
            }
        }
    }
}
