#!/bin/bash

../nullnet-ebpf/target/release/nullnet-user &
./target/release/tun &
