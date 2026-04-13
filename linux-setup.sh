#!/bin/bash

git pull && \
cargo b --release && \
sudo cp nullnet-client.service /etc/systemd/system/ && \
sudo systemctl enable nullnet-client && \
sudo systemctl restart nullnet-client
