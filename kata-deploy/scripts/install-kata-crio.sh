#!/bin/sh

echo "copying kata artifacts onto host"
cp -R /opt/kata-artifacts/opt/kata/* /opt/kata/
chmod +x /opt/kata/bin/*

# Configure crio to use Kata:
echo "Set Kata containers as default runtime in CRI-O for untrusted workloads"
cp /etc/crio/crio.conf /etc/crio/crio.conf.bak
sed -i '/runtime_untrusted_workload = /c\runtime_untrusted_workload = "/opt/kata/bin/kata-runtime"' /etc/crio/crio.conf

echo "Reload systemd services"
systemctl daemon-reload
systemctl restart crio
