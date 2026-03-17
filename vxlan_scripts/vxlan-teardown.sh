#!/bin/bash

# Read CLI arguments:
if [ "$#" -lt 2 ] || [ "$#" -gt 3 ]; then
    echo "Usage: $0 <ns_name> <br_name> [docker_container]"
    echo "Example (standalone): $0 ns_1 br_1"
    echo "Example (docker):     $0 ns_1 br_1 my_container"
    exit 1
fi

NS_NAME=$1
BR_NAME=$2
DOCKER_CONTAINER=$3

# Remove the VXLAN tunnel and veth pair:
sudo ip link set vxlan-$NS_NAME down && sudo ip link del vxlan-$NS_NAME
sudo ip link set $NS_NAME-out down && sudo ip link del $NS_NAME-out

if [ -z "$DOCKER_CONTAINER" ]; then
    # Standalone mode: delete the namespace we created
    # (Docker mode: nothing to do, Docker manages its own namespace)
    sudo ip netns del $NS_NAME
fi

# Remove the bridge:
sudo ip link set $BR_NAME down && sudo ip link del $BR_NAME
