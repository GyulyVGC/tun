#!/bin/bash

# Read CLI arguments:
if [ "$#" -ne 7 ]; then
    echo "Usage: $0 <vxlan_id> <ns_name> <ns_net> <br_name> <br_net> <local_ip> <remote_ip>"
    echo "Example: $0 100 red 10.0.0.1/24 bridge-main 10.0.0.2/24 192.168.1.102 192.168.1.104"
    exit 1
fi

VXLAN_ID=$1
NS_NAME=$2
NS_NET=$3
BR_NAME=$4
BR_NET=$5
LOCAL_IP=$6
REMOTE_IP=$7

BR_IP=$(echo $BR_NET | cut -d'/' -f1)

# Create the namespace and configure the internal interface:
sudo ip netns add $NS_NAME
sudo ip link add $NS_NAME-in type veth peer name $NS_NAME-out
sudo ip link set $NS_NAME-in netns $NS_NAME
sudo ip netns exec $NS_NAME ip addr add $NS_NET dev $NS_NAME-in
sudo ip netns exec $NS_NAME ip link set $NS_NAME-in up
sudo ip netns exec $NS_NAME ip link set lo up

# Create the bridge, assign its internal IP, and attach $NS_NAME-out:
sudo ip link add $BR_NAME type bridge
sudo ip addr add $BR_NET dev $BR_NAME
sudo ip link set $BR_NAME up
sudo ip link set $NS_NAME-out master $BR_NAME
sudo ip link set $NS_NAME-out up
sudo ip netns exec $NS_NAME ip route add default via $BR_IP

# Create the VXLAN tunnel using your physical IP and interface:
sudo ip link add vxlan-$NS_NAME type vxlan id $VXLAN_ID local $LOCAL_IP remote $REMOTE_IP dstport 4789 dev ens18

# Attach the VXLAN to the bridge:
sudo ip link set vxlan-$NS_NAME master $BR_NAME
sudo ip link set vxlan-$NS_NAME up

# Enable IP forwarding:
sudo sysctl -w net.ipv4.ip_forward=1
