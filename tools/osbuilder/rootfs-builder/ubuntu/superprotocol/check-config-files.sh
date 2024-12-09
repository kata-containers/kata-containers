#!/bin/bash

# Define source and destination paths
declare -A files=(
    ["/etc/super/var/lib/rancher/rke2/rke2-pss.yaml"]="/var/lib/rancher/rke2/rke2-pss.yaml"
    ["/etc/super/var/lib/rancher/rke2/server/manifests/k8s.yaml"]="/var/lib/rancher/rke2/server/manifests/k8s.yaml"
    #["/etc/super/var/lib/rancher/rke2/agent/etc/containerd/config.toml.tmpl"]="/var/lib/rancher/rke2/agent/etc/containerd/config.toml.tmpl"
    ["/etc/super/etc/iscsi/iscsid.conf"]="/etc/iscsi/iscsid.conf"
    ["/etc/super/etc/iscsi/initiatorname.iscsi"]="/etc/iscsi/initiatorname.iscsi"
)

# Check and copy files if they do not exist
for src in "${!files[@]}"; do
    dest="${files[$src]}"
    dest_dir=$(dirname "$dest")
    # Create destination directory if it does not exist
    if [ ! -d "$dest_dir" ]; then
        mkdir -p "$dest_dir"
    fi
    # Copy file if it does not exist
    if [ ! -f "$dest" ]; then
        cp -v "$src" "$dest"
    fi
done


K8S="/var/lib/rancher/rke2/server/manifests/k8s.yaml"
ARGO_BRANCH=$(cat /proc/cmdline | grep -o 'argo_branch=[^ ]*' | cut -d= -f2)
if [[ -z "$ARGO_BRANCH" ]]; then
    ARGO_BRANCH="main"
fi
CMDLINE=$(cat /proc/cmdline)
if [[ "$CMDLINE" == *"sp-debug=true"* ]]; then
    sed -i "s/targetRevision: main # argo-vm-selected-branch/targetRevision: $ARGO_BRANCH/" $K8S
else
    echo "k8s.yaml not patched, sp-debug=false"
fi
