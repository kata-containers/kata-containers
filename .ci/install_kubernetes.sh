#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

cidir=$(dirname "$0")
source "${cidir}/lib.sh"

# `use_runtime_class` should be set to:
# - true if we will test using k8s RuntimeClass feature or
# - false (default) if we will test using the old trusted/untrusted annotations.
use_runtime_class=${use_runtime_class:-false}

echo "Install Kubernetes components"

cidir=$(dirname "$0")
source /etc/os-release || source /usr/lib/os-release
kubernetes_version=$(get_version "externals.kubernetes.version")

if [ "$ID" != "ubuntu" ]; then
        echo "Currently this script only works for Ubuntu. Skipped Kubernetes Setup"
        exit
fi

sudo bash -c "cat <<EOF > /etc/apt/sources.list.d/kubernetes.list
deb http://apt.kubernetes.io/ kubernetes-xenial-unstable main
EOF"
curl -s https://packages.cloud.google.com/apt/doc/apt-key.gpg | sudo apt-key add -
chronic sudo -E apt update
chronic sudo -E apt install --allow-downgrades -y kubelet="$kubernetes_version" kubeadm="$kubernetes_version" kubectl="$kubernetes_version"

if [ "${use_runtime_class}"  == true ]; then
	kubelet_systemd_file="/etc/systemd/system/kubelet.service.d/10-kubeadm.conf"
	feature_gate="--feature-gates RuntimeClass=true"
	echo "Configure Kubelet service to enable RuntimeClass feature"
	sudo sed -i "s/ExecStart=\/.*$/& $feature_gate/" "$kubelet_systemd_file"
fi
