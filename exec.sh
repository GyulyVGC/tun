#!/bin/bash

../nullnet-ebpf/target/release/nullnet-user &
./target/release/tun --tun-name tun0 --eth-name ens18 &
