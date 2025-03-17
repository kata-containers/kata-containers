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
CMDLINE="$(cat /proc/cmdline)"
ARGO_BRANCH="main"

if [[ "$CMDLINE" == *"sp-debug=true"* ]]; then
    ARGO_BRANCH_CMDLINE="$(cat /proc/cmdline | grep -o 'argo_branch=[^ ]*' | cut -d= -f2)"
    if [[ -n "$ARGO_BRANCH_CMDLINE" ]]; then
        ARGO_BRANCH="$ARGO_BRANCH_CMDLINE"
    fi
fi

CURRENT_ARGO_BRANCH="$(grep -E 'targetRevision\W+(\w+)' "$K8S" | awk '{print $2}')"
if [[ "$CURRENT_ARGO_BRANCH" != "$ARGO_BRANCH" ]]; then
    echo "Setting $ARGO_BRANCH in $K8S, current: $CURRENT_ARGO_BRANCH"
    sed -ri "s|targetRevision:\W+\w+|targetRevision: $ARGO_BRANCH|" "$K8S";
fi
