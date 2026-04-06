#!/bin/bash

# Read CLI arguments:
if [ "$#" -ne 1 ]; then
    echo "Usage: $0 <ns_name>"
    echo "Example: $0 ns_1"
    exit 1
fi

NS_NAME=$1

sudo ip link set vxlan-$NS_NAME down && sudo ip link del vxlan-$NS_NAME
sudo ip link set $NS_NAME-out down && sudo ip link del $NS_NAME-out

# Delete the namespace if it exists (standalone mode).
# In Docker mode there's no namespace to delete — ip netns del will simply fail silently.
sudo ip netns del $NS_NAME 2>/dev/null
