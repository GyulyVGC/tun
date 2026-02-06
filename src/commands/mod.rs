use ipnetwork::Ipv4Network;
use netlink::NetLinkCommand;
use nullnet_liberror::{Error, ErrorHandler, Location, location};
use ovs::OvsCommand;
use rtnetlink::{Handle, new_connection};

mod netlink;
mod ovs;

pub(crate) async fn setup_br0(rtnetlink_handle: &RtNetLinkHandle) {
    // clean up existing veth interfaces
    rtnetlink_handle
        .execute(NetLinkCommand::DeleteAllVeths)
        .await;

    // delete existing bridge if any
    OvsCommand::DeleteBridge.execute();

    // create the bridge
    OvsCommand::AddBridge.execute();

    // set the bridge up and ovs-system up
    rtnetlink_handle
        .execute(NetLinkCommand::SetInterfacesUp(vec![
            "br0".to_string(),
            "ovs-system".to_string(),
        ]))
        .await;

    // delete existing OpenFlow rules
    OvsCommand::DeleteFlows.execute();

    // use the built-in switching logic
    OvsCommand::AddFlow.execute();

    // add our TAP to the bridge as a trunk port
    OvsCommand::AddTrunkPort.execute();
}

pub(crate) async fn configure_access_port(
    rtnetlink_handle: &RtNetLinkHandle,
    vlan_id: u16,
    net: Ipv4Network,
) {
    rtnetlink_handle
        .execute(NetLinkCommand::HandleVethPairCreation(vlan_id, net))
        .await;
}

#[derive(Clone)]
pub(crate) struct RtNetLinkHandle {
    handle: Handle,
}

impl RtNetLinkHandle {
    pub(crate) async fn new() -> Result<Self, Error> {
        let (rtnetlink_conn, rtnetlink_handle, _) = new_connection().handle_err(location!())?;
        tokio::spawn(rtnetlink_conn);
        Ok(Self {
            handle: rtnetlink_handle,
        })
    }

    async fn execute(&self, command: NetLinkCommand) {
        command.execute(self).await;
    }
}
