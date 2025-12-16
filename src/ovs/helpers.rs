use crate::TAP_NAME;
use ipnetwork::Ipv4Network;
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use nullnet_liberror::{ErrorHandler, Location, location};
use std::process::Command;

pub(super) fn setup_br0() {
    // clean up existing veth interfaces
    delete_all_veths();

    // delete existing bridge if any
    let _ = Command::new("ovs-vsctl")
        .args(["del-br", "br0"])
        .spawn()
        .map(|mut c| c.wait())
        .handle_err(location!());

    // create the bridge
    let _ = Command::new("ovs-vsctl")
        .args(["add-br", "br0"])
        .spawn()
        .map(|mut c| c.wait())
        .handle_err(location!());

    // set the bridge up and ovs-system up
    for dev in &["br0", "ovs-system"] {
        let _ = Command::new("ip")
            .args(["link", "set", dev, "up"])
            .spawn()
            .map(|mut c| c.wait())
            .handle_err(location!());
    }

    // delete existing OpenFlow rules
    let _ = Command::new("ovs-ofctl")
        .args(["del-flows", "br0"])
        .spawn()
        .map(|mut c| c.wait())
        .handle_err(location!());

    // use the built-in switching logic
    let _ = Command::new("ovs-ofctl")
        .args(["add-flow", "br0", "priority=0,actions=normal"])
        .spawn()
        .map(|mut c| c.wait())
        .handle_err(location!());

    // add our TAP to the bridge as a trunk port
    let _ = Command::new("ovs-vsctl")
        .args(["add-port", "br0", TAP_NAME])
        .spawn()
        .map(|mut c| c.wait())
        .handle_err(location!());
}

pub(super) fn configure_access_port(vlan_id: u16, net: Ipv4Network) {
    let ip = net.ip();
    let veth_name = format!("veth{}", ip.to_bits());
    let veth_peer_name = format!("{veth_name}p");

    // delete existing veth pair if any
    let _ = Command::new("ip")
        .args(["link", "del", &veth_name])
        .spawn()
        .map(|mut c| c.wait())
        .handle_err(location!());

    // create veth pair
    let _ = Command::new("ip")
        .args([
            "link",
            "add",
            &veth_name,
            "type",
            "veth",
            "peer",
            "name",
            &veth_peer_name,
        ])
        .spawn()
        .map(|mut c| c.wait())
        .handle_err(location!());

    // set the veth interfaces up
    for dev in &[&veth_name, &veth_peer_name] {
        let _ = Command::new("ip")
            .args(["link", "set", dev, "up"])
            .spawn()
            .map(|mut c| c.wait())
            .handle_err(location!());
    }

    // assign IP address to veth interface
    let _ = Command::new("ip")
        .args(["addr", "add", &net.to_string(), "dev", &veth_name])
        .spawn()
        .map(|mut c| c.wait())
        .handle_err(location!());

    // add the peer interface to the bridge as an access port
    let _ = Command::new("ovs-vsctl")
        .args([
            "add-port",
            "br0",
            &veth_peer_name,
            &format!("tag={vlan_id}"),
        ])
        .spawn()
        .map(|mut c| c.wait())
        .handle_err(location!());
}

fn delete_all_veths() {
    if let Ok(devices) = NetworkInterface::show() {
        for device in devices {
            let name = &device.name;
            if name.starts_with("veth") {
                let _ = Command::new("ip")
                    .args(["link", "del", name])
                    .spawn()
                    .map(|mut c| c.wait())
                    .handle_err(location!());
            }
        }
    }
}
