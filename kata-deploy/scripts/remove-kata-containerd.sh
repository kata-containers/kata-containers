#!/bin/sh
echo "delete kata artifacts"
rm -rf /opt/kata
rm -rf /usr/share/defaults/kata-containers
rm -f /etc/containerd/config.toml

if [ -f /etc/containerd/config.toml.bak ]; then
  mv /etc/containerd/config.toml.bak /etc/containerd/config.toml
fi

