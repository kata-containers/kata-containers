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

# Configure containerd to use Kata:
echo "create containerd configuration for Kata"
mkdir -p /etc/containerd/

if [ -f /etc/containerd/config.toml ]; then
  cp /etc/containerd/config.toml /etc/containerd/config.toml.bak
fi

cat << EOT | tee /etc/containerd/config.toml
[plugins]
    [plugins.cri.containerd]
      [plugins.cri.containerd.untrusted_workload_runtime]
        runtime_type = "io.containerd.runtime.v1.linux"
        runtime_engine = "/opt/kata/bin/kata-runtime"
        runtime_root = ""
EOT

echo "Reload systemd services"
systemctl daemon-reload
systemctl restart containerd
