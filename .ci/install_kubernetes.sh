#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

echo "Install Kubernetes components"

cidir=$(dirname "$0")
source /etc/os-release
kubernetes_version="1.9.3-00"

if [ "$ID" != "ubuntu" ]; then
        echo "Currently this script only works for Ubuntu. Skipped Kubernetes Setup"
        exit
fi

sudo bash -c "cat <<EOF > /etc/apt/sources.list.d/kubernetes.list
deb http://apt.kubernetes.io/ kubernetes-xenial-unstable main
EOF"
curl -s https://packages.cloud.google.com/apt/doc/apt-key.gpg | sudo apt-key add -
sudo -E apt update
sudo -E apt install -y kubelet="$kubernetes_version" kubeadm="$kubernetes_version" kubectl="$kubernetes_version"

echo "Modify kubelet systemd configuration to use CRI-O"
k8s_systemd_file="/etc/systemd/system/kubelet.service.d/10-kubeadm.conf"
sudo sed -i '/KUBELET_AUTHZ_ARGS=/a Environment="KUBELET_EXTRA_ARGS=--container-runtime=remote --container-runtime-endpoint=unix:///var/run/crio/crio.sock --runtime-request-timeout=30m"' "$k8s_systemd_file"
sudo systemctl daemon-reload
