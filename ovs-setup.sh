#!/bin/bash

if [ $# -eq 0 ]
  then
    echo "No IP address supplied"
    exit 1
fi

# set OVS bridge up
sudo ovs-vsctl del-br br0
sudo ovs-vsctl add-br br0
sudo ip link set ovs-system up
sudo ip link set br0 up

# nullnet0 for VLAN 10 (trunk port)
sudo ovs-vsctl add-port br0 nullnet0 trunks=10

# veth pair for VLAN 10 (access port)
sudo ip link add veth10 type veth peer name veth10p
sudo ip link set veth10 up
sudo ip link set veth10p up
sudo ip addr add "$1" dev veth10p
sudo ovs-vsctl add-port br0 veth10 tag=10
# TODO: populate ARP table for veth10p

# delete existing OpenFlow rules
sudo ovs-ofctl -O OpenFlow13 del-flows br0
# OpenFlow rule: veth10 --> nullnet0 with VLAN 10 tagging
sudo ovs-ofctl -O OpenFlow13 add-flow br0 "in_port=2,actions=push_vlan:0x8100,set_vlan_vid:10,output:1"
# OpenFlow rule: nullnet0 --> veth10 with VLAN 10 untagging
sudo ovs-ofctl -O OpenFlow13 add-flow br0 "in_port=1,dl_vlan=10,actions=pop_vlan,output:2"
