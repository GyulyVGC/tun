use crate::commands::ip::IpCommand;
use crate::commands::ovs::OvsCommand;
use ipnetwork::Ipv4Network;

mod ip;
pub mod ovs;

pub(crate) async fn setup_br0() {
    // clean up existing veth interfaces
    IpCommand::DeleteAllVeths.execute().await;

    // delete existing bridge if any
    OvsCommand::DeleteBridge.execute();

    // create the bridge
    OvsCommand::AddBridge.execute();

    // set the bridge up and ovs-system up
    IpCommand::SetInterfacesUp(vec!["br0".to_string(), "ovs-system".to_string()])
        .execute()
        .await;

    // delete existing OpenFlow rules
    OvsCommand::DeleteFlows.execute();

    // use the built-in switching logic
    OvsCommand::AddFlow.execute();

    // add our TAP to the bridge as a trunk port
    OvsCommand::AddTrunkPort.execute();
}

pub(crate) async fn configure_access_port(vlan_id: u16, net: Ipv4Network) {
    IpCommand::HandleVethPairCreation(vlan_id, net)
        .execute()
        .await;
}
