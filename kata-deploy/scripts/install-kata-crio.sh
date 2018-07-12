#!/bin/sh

echo "copying kata artifacts onto host"
cp -R /opt/kata-artifacts/bin /opt/kata/
mkdir /opt/kata/share
mv /opt/kata/bin/qemu /opt/kata/share/
chmod +x /opt/kata/bin/*
cp /opt/kata-artifacts/configuration.toml /usr/share/defaults/kata-containers/configuration.toml

# Update Kata configuration for /opt/kata path usage
sed -i 's!/usr.*kata-containers/!/opt/kata/bin/!' /usr/share/defaults/kata-containers/configuration.toml
sed -i 's!/usr/bin/!/opt/kata/bin/!' /usr/share/defaults/kata-containers/configuration.toml
sed -i 's!qemu-lite!qemu!' /usr/share/defaults/kata-containers/configuration.toml

echo "Set Kata containers as default runtime in CRI-O for untrusted workloads"
cp /etc/crio/crio.conf /etc/crio/crio.conf.bak
sed -i '/runtime_untrusted_workload = /c\runtime_untrusted_workload = "/opt/kata/bin/kata-runtime"' /etc/crio/crio.conf

echo "Reload systemd services"
systemctl daemon-reload
systemctl restart crio
