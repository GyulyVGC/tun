#!/bin/bash

# Read CLI arguments:
if [ "$#" -ne 1 ]; then
    echo "Usage: $0 <ns_name>"
    echo "Example: $0 ns_1"
    exit 1
fi

NS_NAME=$1

sudo ip link set vxlan-$NS_NAME down && ip link del vxlan-$NS_NAME
sudo ip link set $NS_NAME-out down && ip link del $NS_NAME-out
sudo ip netns del $NS_NAME
