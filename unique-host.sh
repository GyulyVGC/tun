#!/bin/bash

set -e

# Must run as root
if [ "$EUID" -ne 0 ]; then
    echo "Please run as root (sudo ./makeLinuxUnique.sh)"
    exit 1
fi


echo "==== System Uniqueness Configuration Tool (Netplan Version) ===="

function ask_yes_no() {
    local prompt="$1"
    local response
    read -p "$prompt (y/n): " response
    [[ "$response" =~ ^[Yy]$ ]]
}

function validate_ip() {
    local ip="$1"
    [[ "$ip" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]
}

# ----------------------------
# 1. Change hostname
# ----------------------------
CURRENT_HOSTNAME=$(hostname)
if ask_yes_no "Current hostname is '$CURRENT_HOSTNAME'. Change it?"; then
    read -p "Enter new hostname: " NEW_HOSTNAME
    if [[ -n "$NEW_HOSTNAME" ]]; then
        if [[ ! "$NEW_HOSTNAME" =~ ^[a-zA-Z0-9][a-zA-Z0-9.-]*$ ]]; then
            echo "Invalid hostname format. Use alphanumeric characters, dots, and hyphens only."
        else
            hostnamectl set-hostname "$NEW_HOSTNAME"
            echo "Hostname changed to $NEW_HOSTNAME"
            sed -i "s/127\.0\.1\.1.*/127.0.1.1\t$NEW_HOSTNAME/" /etc/hosts
        fi
    else
        echo "Invalid hostname. Skipping."
    fi
fi


# ----------------------------
# 2. Regenerate machine-id
# ----------------------------
CURRENT_MACHINE_ID=$(cat /etc/machine-id 2>/dev/null || echo "not set")
if ask_yes_no "Current machine-id is '$CURRENT_MACHINE_ID'. Regenerate?"; then
    echo "Regenerating machine-id..."
    # Remove old IDs and generate a new one for systemd.
    rm -f /etc/machine-id /var/lib/dbus/machine-id
    systemd-machine-id-setup
    # Symlink the D-Bus machine ID to the systemd one as recommended.
    ln -sf /etc/machine-id /var/lib/dbus/machine-id
    echo "New machine-id generated and linked to D-Bus."
fi

# ----------------------------
# 3. Clear DHCP leases
# ----------------------------
if ask_yes_no "Clear DHCP lease files?"; then
    echo "Clearing DHCP leases..."
    rm -f /var/lib/NetworkManager/*.lease /var/lib/dhcp/*.leases
    echo "Old DHCP leases cleared."
fi

# ----------------------------
# 4. Reset random seed
# ----------------------------
if ask_yes_no "Reset system random seed?"; then
    rm -f /var/lib/systemd/random-seed
    echo "Random seed reset."
fi

# ----------------------------
# 5. Static IP Configuration (Netplan)
# ----------------------------
echo ""
echo "Current network interfaces and IPs:"
ip -4 -o addr show | awk '{print $2 ": " $4}'
echo ""

if ask_yes_no "Do you want to configure a static IP via Netplan?"; then
    DEFAULT_IFACE=$(ip route | grep default | awk '{print $5}' | head -n1)
    read -p "Enter network interface (default: ${DEFAULT_IFACE:-ens18}): " IFACE
    IFACE=${IFACE:-${DEFAULT_IFACE:-ens18}}

    if ! ip link show "$IFACE" &>/dev/null; then
        echo "Error: Interface '$IFACE' does not exist."
        exit 1
    fi
    NETPLAN_FILE="/etc/netplan/01-${IFACE}-static.yaml"

    # Gather Current Info for defaults
    CURRENT_IP_FULL=$(ip -4 -o addr show "$IFACE" 2>/dev/null | awk '{print $4}' | head -n1)
    DEFAULT_IP=$(echo "$CURRENT_IP_FULL" | cut -d/ -f1)
    DEFAULT_PREFIX=$(echo "$CURRENT_IP_FULL" | cut -d/ -f2)
    DEFAULT_GW=$(ip route | grep default | grep "$IFACE" | awk '{print $3}' | head -n1)

    echo "--- Configure Settings for $IFACE ---"
    read -p "Enter new static IP address (default: $DEFAULT_IP): " NEW_IP
    NEW_IP=${NEW_IP:-$DEFAULT_IP}
    if ! validate_ip "$NEW_IP"; then
        echo "Error: Invalid IP address format."
        exit 1
    fi

    read -p "Enter subnet prefix (e.g., 24) (default: ${DEFAULT_PREFIX:-24}): " PREFIX
    PREFIX=${PREFIX:-${DEFAULT_PREFIX:-24}}
    if ! [[ "$PREFIX" =~ ^[0-9]+$ ]] || (( PREFIX < 1 || PREFIX > 32 )); then
        echo "Error: Invalid subnet prefix. Must be between 1 and 32."
        exit 1
    fi

    read -p "Enter gateway IP (default: $DEFAULT_GW): " GATEWAY
    GATEWAY=${GATEWAY:-$DEFAULT_GW}
    if [[ -n "$GATEWAY" ]] && ! validate_ip "$GATEWAY"; then
        echo "Error: Invalid gateway IP address format."
        exit 1
    fi

    read -p "Enter DNS server (e.g., 8.8.8.8, 1.1.1.1): " DNS
    DNS=${DNS:-8.8.8.8}
    if ! validate_ip "$DNS"; then
        echo "Error: Invalid DNS server IP address format."
        exit 1
    fi

    echo "Applying changes to $NETPLAN_FILE..."

    # Write the Netplan file
    # Note: We use 'renderer: networkd' for server-style static IPs
    # or 'renderer: NetworkManager' for GUI-integrated management.
    cat <<EOF > "$NETPLAN_FILE"
network:
  version: 2
  renderer: networkd
  ethernets:
    $IFACE:
      dhcp4: no
      addresses: [$NEW_IP/$PREFIX]
      routes:
        - to: default
          via: $GATEWAY
      nameservers:
        addresses: [$DNS]
EOF

    # Secure permissions
    chmod 600 "$NETPLAN_FILE"

    # Apply Netplan and verify
    for i in {1..2}; do
        echo "Attempt $i: Applying Netplan configuration..."
        netplan apply
        # Allow some time for the network to settle
        sleep 5

        CURRENT_IP_FULL=$(ip -4 -o addr show "$IFACE" 2>/dev/null | awk '{print $4}' | head -n1)
        CURRENT_IP=$(echo "$CURRENT_IP_FULL" | cut -d/ -f1)

        if [[ "$CURRENT_IP" == "$NEW_IP" ]]; then
            echo "Network configuration applied successfully. Current IP: $CURRENT_IP"
            break
        fi

        if (( i == 2 )); then
            echo "Error: Failed to apply Netplan configuration after $i attempts. Please check the syntax in $NETPLAN_FILE"
            exit 1
        fi
    done
fi

echo ""
echo "System uniqueness changes complete."
if ask_yes_no "Reboot now?"; then
    echo "Rebooting now..."
    reboot
fi