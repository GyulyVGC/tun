#!/bin/bash

../nullnet-ebpf/target/release/nullnet-user --tun-name tun0 --eth-name ens18 &
./target/release/tun &
