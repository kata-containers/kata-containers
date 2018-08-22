#!/bin/sh

echo "copying kata artifacts onto host"
cp -R /opt/kata-artifacts/opt/kata/* /opt/kata/
chmod +x /opt/kata/bin/*

# Configure containerd to use Kata:
echo "create containerd configuration for Kata"
mkdir -p /etc/containerd/

if [ -f /etc/containerd/config.toml ]; then
	cp /etc/containerd/config.toml /etc/containerd/config.toml.bak
fi

cat <<EOT | tee /etc/containerd/config.toml
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
