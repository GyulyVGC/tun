#!/bin/bash

# Update nullnet.service file
cp nullnet.service /etc/systemd/system/ && \
systemctl enable nullnet && \
git checkout main && \
git pull && \
cargo b --release && \
reboot
