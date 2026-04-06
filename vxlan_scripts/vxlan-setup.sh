#!/bin/bash

# Read CLI arguments:
if [ "$#" -lt 7 ] || [ "$#" -gt 8 ]; then
    echo "Usage: $0 <vxlan_id> <ns_name> <ns_net> <br_name> <br_net> <local_ip> <remote_ip> [docker_container]"
    echo "Example (standalone): $0 100 ns_100_s 10.0.0.1/29 br_100_s 10.0.0.2/29 192.168.1.102 192.168.1.104"
    echo "Example (docker):     $0 100 ns_100_s 10.0.0.1/29 br_100_s 10.0.0.2/29 192.168.1.102 192.168.1.104 my_container"
    exit 1
fi

VXLAN_ID=$1
NS_NAME=$2
NS_NET=$3
BR_NAME=$4
BR_NET=$5
LOCAL_IP=$6
REMOTE_IP=$7
DOCKER_CONTAINER=$8

BR_IP=$(echo $BR_NET | cut -d'/' -f1)

if [ -n "$DOCKER_CONTAINER" ]; then
    # Docker mode: get the container's PID to enter its network namespace via nsenter
    PID=$(docker inspect -f '{{.State.Pid}}' $DOCKER_CONTAINER)
    NS_EXEC="sudo nsenter -t $PID -n"
    # Move a veth into the container's namespace using its PID
    NS_SET="sudo ip link set $NS_NAME-in netns $PID"
else
    # Standalone mode: create a new network namespace
    sudo ip netns add $NS_NAME
    NS_EXEC="sudo ip netns exec $NS_NAME"
    NS_SET="sudo ip link set $NS_NAME-in netns $NS_NAME"
fi

# Create a veth pair and move one end into the namespace:
sudo ip link add $NS_NAME-in type veth peer name $NS_NAME-out
$NS_SET
$NS_EXEC ip addr add $NS_NET dev $NS_NAME-in
$NS_EXEC ip link set $NS_NAME-in up

# Create the bridge, assign its internal IP, and attach $NS_NAME-out:
sudo ip link add $BR_NAME type bridge
sudo ip addr add $BR_NET dev $BR_NAME
sudo ip link set $BR_NAME up
sudo ip link set $NS_NAME-out master $BR_NAME
sudo ip link set $NS_NAME-out up
if [ -z "$DOCKER_CONTAINER" ]; then
    # Standalone mode: set default route through the bridge
    $NS_EXEC ip route add default via $BR_IP
fi

if [ "$LOCAL_IP" == "$REMOTE_IP" ]; then
      # Same host: connect bridges with a veth pair instead of a VXLAN tunnel
      VETH_S="veth-${VXLAN_ID}-s"
      VETH_C="veth-${VXLAN_ID}-c"
      # Both ends are created atomically; the losing task's EEXIST is harmless
      sudo ip link add "$VETH_S" type veth peer name "$VETH_C" 2>/dev/null
      # Attach our end to our bridge
      if [[ "$BR_NAME" == *_s ]]; then
          LOCAL_VETH="$VETH_S"
      else
          LOCAL_VETH="$VETH_C"
      fi
      sudo ip link set "$LOCAL_VETH" master "$BR_NAME"
      sudo ip link set "$LOCAL_VETH" up
  else
      # Create the VXLAN tunnel using your physical IP and interface:
      sudo ip link add vxlan-$NS_NAME type vxlan id $VXLAN_ID local $LOCAL_IP remote $REMOTE_IP dstport 4789 # dev ens18
      # Attach the VXLAN to the bridge:
      sudo ip link set vxlan-$NS_NAME master $BR_NAME
      sudo ip link set vxlan-$NS_NAME up
  fi

# Enable IP forwarding:
sudo sysctl -w net.ipv4.ip_forward=1

# Allow forwarding (Docker sets FORWARD policy to DROP):
sudo iptables -P FORWARD ACCEPT
