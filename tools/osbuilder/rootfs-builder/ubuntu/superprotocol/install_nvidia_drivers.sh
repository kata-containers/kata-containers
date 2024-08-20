#!/bin/bash

mkdir -p /var/cache/apt/archives/partial
mkdir -p /var/log/apt
dpkg --configure -a
apt update

DEBIAN_FRONTEND=noninteractive apt install libc6 libc-bin --reinstall --no-install-recommends -y

cd /opt/deb/nvidia
dpkg -i *.deb
apt update
DEBIAN_FRONTEND=noninteractive apt install -y --no-install-recommends nvidia-driver-555-open

cd /opt/deb
dpkg -i *.deb

rm -rf /var/log/apt
rm -rf /var/cache/apt/archives/partial
rm -rf /opt/deb