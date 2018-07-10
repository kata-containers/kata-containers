#!/bin/sh
echo "copying kata artifacts from /tmp to /opt"
cp -R /tmp/kata/* /opt/kata/

chmod +x /opt/kata/bin/*

cp /opt/kata/configuration.toml /usr/share/defaults/kata-containers/configuration.toml

cp /etc/crio/crio.conf /etc/crio/crio.conf.bak

echo "Set Kata containers as default runtime in CRI-O for untrusted workloads"
sed -i '/runtime_untrusted_workload = /c\runtime_untrusted_workload = "/opt/kata/bin/kata-runtime"' /etc/crio/crio.conf

echo "Reload systemd services"
systemctl daemon-reload
systemctl restart crio
