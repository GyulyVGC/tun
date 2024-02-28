#!/bin/bash

# Update nullnet.service file
sudo cp nullnet.service /etc/systemd/system/ && \
sudo systemctl enable nullnet && \
git checkout main && \
git pull && \
cargo b --release && \
sudo reboot
