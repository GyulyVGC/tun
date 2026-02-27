#!/bin/bash

# Read CLI arguments:
if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <ns_name> <br_name>"
    echo "Example: $0 ns1 br1"
    exit 1
fi

NS_NAME=$1
BR_NAME=$2

sudo ip link set vxlan-$NS_NAME down && ip link del vxlan-$NS_NAME
sudo ip link set $NS_NAME-out down && ip link del $NS_NAME-out
sudo ip netns del $NS_NAME
sudo ip link set $BR_NAME down && ip link del $BR_NAME
