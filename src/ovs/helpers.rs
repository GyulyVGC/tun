use ipnetwork::Ipv4Network;
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use std::process::Command;

pub(super) fn setup_br0() {
    // remove nullnet0 from the bridge
    let res = Command::new("ovs-vsctl")
        .args(&["del-port", "br0", "nullnet0"])
        .spawn();
    if let Ok(mut child) = res {
        let _ = child.wait();
    }

    // delete existing bridge if any
    let res = Command::new("ovs-vsctl").args(&["del-br", "br0"]).spawn();
    if let Ok(mut child) = res {
        let _ = child.wait();
    }

    // create the bridge
    let res = Command::new("ovs-vsctl").args(&["add-br", "br0"]).spawn();
    if let Ok(mut child) = res {
        let _ = child.wait();
    }

    // set the bridge up and ovs-system up
    for dev in &["br0", "ovs-system"] {
        let res = Command::new("ip").args(&["link", "set", dev, "up"]).spawn();
        if let Ok(mut child) = res {
            let _ = child.wait();
        }
    }

    // delete existing OpenFlow rules
    let res = Command::new("ovs-ofctl")
        .args(&["del-flows", "br0"])
        .spawn();
    if let Ok(mut child) = res {
        let _ = child.wait();
    }

    // use the built-in switching logic
    let res = Command::new("ovs-ofctl")
        .args(&["add-flow", "br0", "priority=0,actions=normal"])
        .spawn();
    if let Ok(mut child) = res {
        let _ = child.wait();
    }

    // add nullnet0 to the bridge as a trunk port
    let res = Command::new("ovs-vsctl")
        .args(&["add-port", "br0", "nullnet0"])
        .spawn();
    if let Ok(mut child) = res {
        let _ = child.wait();
    }
}

pub(super) fn configure_access_port(vlan_id: u16, net: &Ipv4Network) {
    let ip = net.ip();
    let veth_name = format!("veth{}", ip.to_bits());
    let veth_peer_name = format!("{veth_name}p");

    // delete existing veth pair if any
    let res = Command::new("ip")
        .args(&["link", "del", &veth_name])
        .spawn();
    if let Ok(mut child) = res {
        let _ = child.wait();
    }

    // create veth pair
    let res = Command::new("ip")
        .args(&[
            "link",
            "add",
            &veth_name,
            "type",
            "veth",
            "peer",
            "name",
            &veth_peer_name,
        ])
        .spawn();
    if let Ok(mut child) = res {
        let _ = child.wait();
    }

    // set the veth interfaces up
    for dev in &[&veth_name, &veth_peer_name] {
        let res = Command::new("ip").args(&["link", "set", dev, "up"]).spawn();
        if let Ok(mut child) = res {
            let _ = child.wait();
        }
    }

    // assign IP address to veth interface
    let res = Command::new("ip")
        .args(&["addr", "add", &net.to_string(), "dev", &veth_name])
        .spawn();
    if let Ok(mut child) = res {
        let _ = child.wait();
    }

    // add the peer interface to the bridge as an access port
    let res = Command::new("ovs-vsctl")
        .args(&[
            "add-port",
            "br0",
            &veth_peer_name,
            &format!("tag={vlan_id}"),
        ])
        .spawn();
    if let Ok(mut child) = res {
        let _ = child.wait();
    }
}

pub(crate) fn delete_all_veths() {
    if let Ok(devices) = NetworkInterface::show() {
        for device in devices {
            let name = &device.name;
            if name.starts_with("veth") {
                let res = Command::new("ip")
                    .args(&["link", "del", name])
                    .spawn();
                if let Ok(mut child) = res {
                    let _ = child.wait();
                }
            }
        }
    }
}
