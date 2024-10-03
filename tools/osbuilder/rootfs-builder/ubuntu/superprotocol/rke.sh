#!/bin/bash
set -x

RND_SEED=$(LC_ALL=C tr -dc '[:lower:]' < /dev/urandom | head -c 6)
NODE_NAME="sp-tdx-h100-vm-$RND_SEED"
echo $NODE_NAME > /etc/hostname

LOCAL_REGISTRY_HOST="hauler.local"
SUPER_REGISTRY_HOST="registry.superprotocol.local"
SUPER_SCRIPT_DIR="/etc/super"
mkdir -p "$SUPER_SCRIPT_DIR"

mkdir -p "/etc/rancher/rke2"
cat > "/etc/rancher/rke2/config.yaml" <<EOF
kubelet-arg:
  - max-pods=256
disable:
  - rke2-ingress-nginx
  - rke2-metrics-server
cni:
  - cilium
#system-default-registry: $LOCAL_REGISTRY_HOST
node-label:
  - node.tee.superprotocol.com/name=$NODE_NAME
EOF
cat > "/etc/rancher/rke2/registries.yaml" <<EOF
configs:
  "$SUPER_REGISTRY_HOST:32443":
    tls:
      insecure_skip_verify: true
  "$LOCAL_REGISTRY_HOST:5000":
    tls:
      insecure_skip_verify: true
mirrors:
  "*":
    endpoint:
      - "http://$LOCAL_REGISTRY_HOST:5000"
EOF

mkdir -p "/etc/cni/net.d"
cat > "/etc/cni/net.d/05-cilium.conflist" <<EOF
{
  "cniVersion": "0.3.1",
  "name": "portmap",
  "plugins": [
    {
       "type": "cilium-cni",
       "enable-debug": false,
       "log-file": "/var/run/cilium/cilium-cni.log"
    },
    {
      "type": "portmap",
      "capabilities": {"portMappings": true}
    }
  ]
}
EOF

mkdir -p "/etc/sysctl.d/"
cat > "/etc/sysctl.d/99-zzz-override_cilium.conf" <<EOF
# Disable rp_filter on Cilium interfaces since it may cause mangled packets to be dropped
-net.ipv4.conf.lxc*.rp_filter = 0
-net.ipv4.conf.cilium_*.rp_filter = 0
# The kernel uses max(conf.all, conf.{dev}) as its value, so we need to set .all. to 0 as well.
# Otherwise it will overrule the device specific settings.
net.ipv4.conf.all.rp_filter = 0
EOF

mkdir -p "/etc/rancher/node"
LC_ALL=C tr -dc '[:alpha:][:digit:]' </dev/urandom | head -c 32 > /etc/rancher/node/password

# Install rke2
vRKE2=v1.30.3+rke2r1

mkdir -p /root/rke2-artifacts
cd /root/rke2-artifacts/
curl -OLs "https://github.com/rancher/rke2/releases/download/${vRKE2}/rke2-images.linux-amd64.tar.zst"
curl -OLs "https://github.com/rancher/rke2/releases/download/${vRKE2}/rke2.linux-amd64.tar.gz"
curl -OLs "https://github.com/rancher/rke2/releases/download/${vRKE2}/sha256sum-amd64.txt"
curl -sfL https://get.rke2.io --output install.sh

# for v1.30.3+rke2r1
SHA_CHECKSUMS=0019dfc4b32d63c1392aa264aed2253c1e0c2fb09216f8e2cc269bbfb8bb49b5
SHA_INSTALL=8d57ffcda9974639891af35a01e9c3c2b8f97ac71075a805d60060064b054492

echo "$SHA_CHECKSUMS sha256sum-amd64.txt" | sha256sum --check
echo "$SHA_INSTALL install.sh" | sha256sum --check

INSTALL_RKE2_ARTIFACT_PATH=/root/rke2-artifacts sh install.sh

cd -
systemctl enable rke2-server.service

mkdir -p "/var/lib/rancher/rke2"
mkdir -p "$SUPER_SCRIPT_DIR/var/lib/rancher/rke2"
#cat > "/etc/rancher/rke2/rke2-pss.yaml" <<EOF
cat > "$SUPER_SCRIPT_DIR/var/lib/rancher/rke2/rke2-pss.yaml" <<EOF
apiVersion: apiserver.config.k8s.io/v1
kind: AdmissionConfiguration
plugins:
- name: PodSecurity
  configuration:
    apiVersion: pod-security.admission.config.k8s.io/v1beta1
    kind: PodSecurityConfiguration
    defaults:
      enforce: "privileged"
      enforce-version: "latest"
    exemptions:
      usernames: []
      runtimeClasses: []
      namespaces: []
EOF

mkdir -p "$SUPER_SCRIPT_DIR/var/lib/rancher/rke2/agent/etc/containerd/"
cat > "$SUPER_SCRIPT_DIR/var/lib/rancher/rke2/agent/etc/containerd/config.toml.tmpl" <<EOF
version = 2
[plugins."io.containerd.internal.v1.opt"]
  path = "/var/lib/rancher/rke2/agent/containerd"
[plugins."io.containerd.grpc.v1.cri"]
  stream_server_address = "127.0.0.1"
  stream_server_port = "10010"
  enable_selinux = false
  enable_unprivileged_ports = true
  enable_unprivileged_icmp = true
  sandbox_image = "index.docker.io/rancher/mirrored-pause:3.6"
  [plugins."io.containerd.grpc.v1.cri".registry]
    [plugins."io.containerd.grpc.v1.cri".registry.mirrors]
      [plugins."io.containerd.grpc.v1.cri".registry.mirrors."$SUPER_REGISTRY_HOST:32443"]
        endpoint = ["https://$SUPER_REGISTRY_HOST:32443"]
      [plugins."io.containerd.grpc.v1.cri".registry.configs."$SUPER_REGISTRY_HOST:32443".tls]
        insecure_skip_verify = true
      [plugins."io.containerd.grpc.v1.cri".registry.mirrors."$LOCAL_REGISTRY_HOST:5000"]
        endpoint = ["https://$LOCAL_REGISTRY_HOST:5000"]
      [plugins."io.containerd.grpc.v1.cri".registry.configs."$LOCAL_REGISTRY_HOST:5000".tls]
        insecure_skip_verify = true
  [plugins."io.containerd.grpc.v1.cri".containerd]
    snapshotter = "overlayfs"
    disable_snapshot_annotations = true
    default_runtime_name = "nvidia"
    [plugins."io.containerd.grpc.v1.cri".containerd.runtimes]
      [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.runc]
        runtime_type = "io.containerd.runc.v2"
        [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.runc.options]
          SystemdCgroup = true
      [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.nvidia]
        privileged_without_host_devices = false
        runtime_engine = ""
        runtime_root = ""
        runtime_type = "io.containerd.runc.v2"
        [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.nvidia.options]
          BinaryName = "/opt/nvidia/toolkit/nvidia-container-runtime"
          SystemdCgroup = true
      [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.nvidia-cdi]
        privileged_without_host_devices = false
        runtime_engine = ""
        runtime_root = ""
        runtime_type = "io.containerd.runc.v2"
        [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.nvidia-cdi.options]
          BinaryName = "/opt/nvidia/toolkit/nvidia-container-runtime.cdi"
      [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.nvidia-legacy]
        privileged_without_host_devices = false
        runtime_engine = ""
        runtime_root = ""
        runtime_type = "io.containerd.runc.v2"
        [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.nvidia-legacy.options]
          BinaryName = "/opt/nvidia/toolkit/nvidia-container-runtime.legacy"
EOF

cat >> /usr/local/lib/systemd/system/rke2-server.env <<EOF
RKE2_KUBECONFIG_OUTPUT=/var/lib/rancher/rke2/rke2.yaml
RKE2_POD_SECURITY_ADMISSION_CONFIG_FILE=/var/lib/rancher/rke2/rke2-pss.yaml
EOF

# fix problem with PVC multi-attach https://longhorn.io/kb/troubleshooting-volume-with-multipath/
cat >> /etc/multipath.conf <<EOF
blacklist {
    devnode "^sd[a-z0-9]+"
}
EOF

# copy iscsi configs, cause this partition will be remounted with empty dir
mkdir -p "$SUPER_SCRIPT_DIR/etc/iscsi/"
cp -r "/etc/iscsi/" "$SUPER_SCRIPT_DIR/etc/"

mkdir -p /etc/kubernetes

cat > /etc/resolv.conf <<EOF
nameserver 127.0.0.53
nameserver 1.1.1.1
nameserver 8.8.8.8
options edns0 trust-ad
search .
EOF

cat >> /etc/hosts <<EOF
10.0.2.15	$SUPER_REGISTRY_HOST $LOCAL_REGISTRY_HOST
EOF

# debug
#echo "stty cols 180 rows 50" >> /etc/profile

echo "export KUBECONFIG=/var/lib/rancher/rke2/rke2.yaml" >>  /etc/profile
echo "alias k='/var/lib/rancher/rke2/bin/kubectl'" >>  /etc/profile
echo "alias kubectl='/var/lib/rancher/rke2/bin/kubectl'" >>  /etc/profile

sed -i 's|[#]*PasswordAuthentication .*|PasswordAuthentication yes|g' /etc/ssh/sshd_config
sed -i 's|[#]*PermitRootLogin .*|PermitRootLogin yes|g' /etc/ssh/sshd_config
sed -i 's|[#]*KbdInteractiveAuthentication .*|KbdInteractiveAuthentication yes|g' /etc/ssh/sshd_config

# HAULER
### Setup Directories
mkdir -p /opt/hauler/.hauler
cd /opt/hauler

ln -s /opt/hauler/.hauler ~/.hauler

### Download and Install Hauler
vHauler=1.0.8
curl -sfL https://get.hauler.dev | HAULER_VERSION=${vHauler} bash

### Fetch Rancher Airgap Manifests
cat > "rke2-airgap.yaml" <<EOF
apiVersion: content.hauler.cattle.io/v1alpha1
kind: Images
metadata:
  name: rke2-airgap
spec:
  images:
    - name: rancher/backup-restore-operator:v5.0.1
    - name: rancher/cis-operator:v1.0.14
    - name: rancher/flannel-cni:v1.4.1-rancher1
    - name: rancher/fleet-agent:v0.10.1
    - name: rancher/fleet:v0.10.1
    - name: rancher/hardened-addon-resizer:1.8.20-build20240410
    - name: rancher/hardened-cluster-autoscaler:v1.8.10-build20240124
    - name: rancher/hardened-cni-plugins:v1.4.1-build20240325
    - name: rancher/hardened-coredns:v1.11.1-build20240305
    - name: rancher/hardened-dns-node-cache:1.22.28-build20240125
    - name: rancher/hardened-etcd:v3.5.13-k3s1-build20240531
    - name: rancher/hardened-flannel:v0.25.4-build20240610
    - name: rancher/hardened-k8s-metrics-server:v0.7.1-build20240401
    - name: rancher/hardened-kubernetes:v1.30.3-rke2r1-build20240717
    - name: rancher/hardened-node-feature-discovery:v0.15.4-build20240513
    - name: rancher/hardened-whereabouts:v0.7.0-build20240429
    - name: rancher/helm-project-operator:v0.2.1
    - name: rancher/k3s-upgrade:v1.30.3-k3s1
    - name: rancher/klipper-helm:v0.8.4-build20240523
    - name: rancher/klipper-lb:v0.4.7
    - name: rancher/kube-api-auth:v0.2.2
    - name: rancher/kubectl:v1.29.7
    - name: rancher/local-path-provisioner:v0.0.28
    - name: rancher/machine:v0.15.0-rancher116
    - name: rancher/mirrored-cilium-certgen:v0.1.12
    - name: rancher/mirrored-cilium-cilium-envoy:v1.28.3-31ec52ec5f2e4d28a8e19a0bfb872fa48cf7a515
    - name: rancher/mirrored-cilium-cilium-etcd-operator:v2.0.7
    - name: rancher/mirrored-cilium-cilium:v1.15.5
    - name: rancher/mirrored-cilium-clustermesh-apiserver:v1.15.5
    - name: rancher/mirrored-cilium-hubble-relay:v1.15.5
    - name: rancher/mirrored-cilium-hubble-ui-backend:v0.13.0
    - name: rancher/mirrored-cilium-hubble-ui:v0.13.0
    - name: rancher/mirrored-cilium-kvstoremesh:v1.14.4
    - name: rancher/mirrored-cilium-operator-aws:v1.15.5
    - name: rancher/mirrored-cilium-operator-azure:v1.15.5
    - name: rancher/mirrored-cilium-operator-generic:v1.15.5
    - name: rancher/mirrored-kiali-kiali:v1.86.0
    - name: rancher/mirrored-kiwigrid-k8s-sidecar:1.26.1
    - name: rancher/mirrored-kube-logging-logging-operator:4.8.0
    - name: rancher/mirrored-kube-rbac-proxy:v0.15.0
    - name: rancher/mirrored-kube-state-metrics-kube-state-metrics:v2.10.1
    - name: rancher/mirrored-library-busybox:1.36.1
    - name: rancher/mirrored-library-nginx:1.24.0-alpine
    - name: rancher/mirrored-metrics-server:v0.7.1
    - name: rancher/mirrored-nginx-ingress-controller-defaultbackend:1.5-rancher1
    - name: rancher/mirrored-pause:3.6
    - name: rancher/mirrored-pause:3.7
    - name: rancher/mirrored-sig-storage-csi-attacher:v4.5.1
    - name: rancher/mirrored-sig-storage-csi-node-driver-registrar:v2.10.1
    - name: rancher/mirrored-sig-storage-csi-provisioner:v4.0.1
    - name: rancher/mirrored-sig-storage-csi-resizer:v1.10.1
    - name: rancher/mirrored-sig-storage-csi-snapshotter:v7.0.2
    - name: rancher/mirrored-sig-storage-livenessprobe:v2.12.0
    - name: rancher/mirrored-sig-storage-snapshot-controller:v6.2.1
    - name: rancher/mirrored-sig-storage-snapshot-validation-webhook:v6.2.2
    - name: rancher/nginx-ingress-controller:v1.10.1-hardened1
    - name: rancher/pushprox-client:v0.1.3-rancher2-client
    - name: rancher/pushprox-proxy:v0.1.3-rancher2-proxy
    - name: rancher/rke-tools:v0.1.100
    - name: rancher/rke2-cloud-provider:v1.29.3-build20240515
    - name: rancher/rke2-runtime:v1.30.3-rke2r1
    - name: rancher/rke2-upgrade:v1.30.3-rke2r1
    - name: rancher/security-scan:v0.2.16
    - name: rancher/shell:v0.2.1
    - name: rancher/system-agent-installer-k3s:v1.30.3-k3s1
    - name: rancher/system-agent-installer-rke2:v1.30.3-rke2r1
    - name: rancher/system-agent:v0.3.8-suc
    - name: rancher/system-upgrade-controller:v0.13.4
EOF

hauler store sync --store rke2-store --platform linux/amd64 --files rke2-airgap.yaml
hauler store save --store rke2-store --filename rke2-airgap.tar.zst
# @TODO add argo-cd, argo-workflows, cert-manager, gpu-operator, longhorn charts

mkdir -p $SUPER_SCRIPT_DIR/opt/hauler
cp *.tar.zst $SUPER_SCRIPT_DIR/opt/hauler/
rm -rf /opt/hauler/*
