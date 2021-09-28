#!/bin/bash

# Setup a simple fake network (Host -> VM) that looks like SLIRP
# Run a DHCP Server so that the VM can get an IP Automatically
sudo ip link del clhtap
sudo ip link del clhbr
sudo ip tuntap add mode tap clhtap
sudo ip link set dev clhtap up
sudo ip link add clhbr type bridge
sudo ip link set clhtap master clhbr
sudo ip addr add 10.0.2.1/24 dev clhbr
sudo ip link set dev clhbr up
sudo ip link set dev clhtap up

echo "Starting DHCP Server"
sudo dnsmasq --conf-file=dnsmasq.conf --no-daemon 
