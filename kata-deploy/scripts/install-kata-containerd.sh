#!/bin/sh
echo "copying kata artifacts onto host"
cp -R /tmp/kata/* /opt/kata/
chmod +x /opt/kata/bin/*
cp /opt/kata/configuration.toml /usr/share/defaults/kata-containers/configuration.toml

## Configure containerd to use Kata:
echo "create containerd configuration for Kata"
mkdir -p /etc/containerd/

cat << EOT | tee /etc/containerd/config.toml
[plugins]
    [plugins.cri.containerd]
      snapshotter = "overlayfs"
      [plugins.cri.containerd.untrusted_workload_runtime]
        runtime_type = "io.containerd.runtime.v1.linux"
        runtime_engine = "/opt/kata/bin/kata-runtime"
        runtime_root = ""
EOT

echo "Reload systemd services"
systemctl daemon-reload
systemctl restart containerd
