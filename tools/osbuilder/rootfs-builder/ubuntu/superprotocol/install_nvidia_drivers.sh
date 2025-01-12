#!/bin/bash

apt update

cd /opt/deb/nvidia
dpkg -i *.deb
apt update
DEBIAN_FRONTEND=noninteractive apt install -y --no-install-recommends nvidia-driver-565-server-open
cd /opt/deb
dpkg -i *.deb
