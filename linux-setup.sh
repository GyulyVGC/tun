#!/bin/bash

# Update tun.service file
sudo cp tun.service /etc/systemd/system/ && \
sudo systemctl enable tun && \
git pull && \
cargo b --release && \
sudo reboot
