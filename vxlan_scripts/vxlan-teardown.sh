#!/bin/bash

# Read CLI arguments:
if [ "$#" -lt 3 ] || [ "$#" -gt 4 ]; then
    echo "Usage: $0 <vxlan_id> <ns_name> <br_name> [docker_container]"
    echo "Example (standalone): $0 100 ns_100_s br_100_s"
    echo "Example (docker):     $0 100 ns_100_s br_100_s my_container"
    exit 1
fi

VXLAN_ID=$1
NS_NAME=$2
BR_NAME=$3
DOCKER_CONTAINER=$4

# Remove the VXLAN tunnel or same-host veth pair:
sudo ip link set vxlan-$NS_NAME down && sudo ip link del vxlan-$NS_NAME
sudo ip link set veth-${VXLAN_ID}-s down && sudo ip link del veth-${VXLAN_ID}-s

# Remove the namespace veth pair:
sudo ip link set $NS_NAME-out down && sudo ip link del $NS_NAME-out

if [ -z "$DOCKER_CONTAINER" ]; then
    # Standalone mode: delete the namespace we created
    # (Docker mode: nothing to do, Docker manages its own namespace)
    sudo ip netns del $NS_NAME
fi

# Remove the bridge:
sudo ip link set $BR_NAME down && sudo ip link del $BR_NAME
