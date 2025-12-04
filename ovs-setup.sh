#!/bin/bash

if [ $# -eq 0 ]
  then
    echo "No IP address/subnet supplied for the veth10p interface"
    exit 1
fi

if [ $# -gt 1 ]
  then
    echo "Too many arguments supplied (only supply the IP address/subnet for the veth10p interface)"
    exit 1
fi

# set OVS bridge up
sudo ovs-vsctl del-br br0 >/dev/null 2>&1
sudo ovs-vsctl add-br br0
sudo ip link set ovs-system up
sudo ip link set br0 up

# delete existing OpenFlow rules on the bridge and use the built-in switching logic
sudo ovs-ofctl del-flows br0
sudo ovs-ofctl add-flow br0 "priority=0,actions=normal"

# set nullnet0 as a trunk port
sudo ovs-vsctl add-port br0 nullnet0

# create and set veth pair as access port for VLAN 10
sudo ip link del veth10 >/dev/null 2>&1
sudo ip link add veth10 type veth peer name veth10p
sudo ip link set veth10 up
sudo ip link set veth10p up
sudo ip addr add "$1" dev veth10
sudo ovs-vsctl add-port br0 veth10p tag=10

# ----------------------------------------------------------------------------------------------------------------------
# OpenFlow rule: veth10 --> nullnet0 with VLAN 10 tagging
#sudo ovs-ofctl -O OpenFlow13 add-flow br0 "in_port=2,actions=push_vlan:0x8100,set_vlan_vid:10,output:1"
# OpenFlow rule: nullnet0 --> veth10 with VLAN 10 untagging
#sudo ovs-ofctl -O OpenFlow13 add-flow br0 "in_port=1,dl_vlan=10,actions=pop_vlan,output:2"
# ----------------------------------------------------------------------------------------------------------------------
