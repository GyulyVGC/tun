#!/bin/bash

# Update nullnet.service file
cp nullnet.service /etc/systemd/system/
systemctl enable nullnet

# Build latest version of the executable
git checkout main
git pull
cargo b --release

# Reboot
reboot
