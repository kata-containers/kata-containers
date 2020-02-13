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
KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"

echo "Install Kubernetes components"

cidir=$(dirname "$0")
source /etc/os-release || source /usr/lib/os-release
kubernetes_version=$(get_version "externals.kubernetes.version")

if [ "$ID" != "ubuntu" ] && [ "$ID" != "centos" ]; then
        echo "Currently this script does not work on $ID. Skipped Kubernetes Setup"
        exit 0
fi

if [ "$KATA_HYPERVISOR" == "firecracker" ]; then
	die "Kubernetes will not work with $KATA_HYPERVISOR"
fi

if [ "$ID" == "ubuntu" ]; then
	sudo bash -c "cat <<EOF > /etc/apt/sources.list.d/kubernetes.list
	deb http://apt.kubernetes.io/ kubernetes-xenial-unstable main
EOF"

	chronic sudo -E sed -i 's/^[ \t]*//' /etc/apt/sources.list.d/kubernetes.list
	curl -s https://packages.cloud.google.com/apt/doc/apt-key.gpg | sudo apt-key add -
	chronic sudo -E apt update
	chronic sudo -E apt install --allow-downgrades -y kubelet="$kubernetes_version" kubeadm="$kubernetes_version" kubectl="$kubernetes_version"
elif [ "$ID" == "centos" ]; then
	if [ "$ID" == "centos" ]; then
		sudo yum versionlock docker-ce
	fi

	sudo bash -c "cat <<EOF > /etc/yum.repos.d/kubernetes.repo
	[kubernetes]
	name=Kubernetes
	baseurl=https://packages.cloud.google.com/yum/repos/kubernetes-el7-x86_64
	enabled=1
	gpgcheck=1
	repo_gpgcheck=1
	gpgkey=https://packages.cloud.google.com/yum/doc/yum-key.gpg https://packages.cloud.google.com/yum/doc/rpm-package-key.gpg
EOF"

	chronic sudo -E sed -i 's/^[ \t]*//' /etc/yum.repos.d/kubernetes.repo
	install_kubernetes_version=$(echo $kubernetes_version | cut -d'-' -f1)
	chronic sudo -E yum -y update
	chronic sudo -E yum install -y kubelet-"$install_kubernetes_version" kubeadm-"$install_kubernetes_version" kubectl-"$install_kubernetes_version" --disableexcludes=kubernetes

	# Disable selinux
	if  [ "$(getenforce)" != "Disabled" ]; then
		chronic sudo -E setenforce 0
		chronic sudo -E sed -i 's/^SELINUX=enforcing/SELINUX=disabled/g' /etc/sysconfig/selinux
	fi

	# Packets traversing the bridge should be sent to iptables for processing
	echo br_netfilter | sudo -E tee /etc/modules-load.d/k8s.conf
	sudo -E modprobe -i br_netfilter
	sudo -E bash -c 'echo "net.bridge.bridge-nf-call-ip6tables = 1" > /etc/sysctl.d/k8s.conf'
	sudo -E bash -c 'echo "net.bridge.bridge-nf-call-iptables = 1" >> /etc/sysctl.d/k8s.conf'
	sudo -E bash -c 'echo "net.ipv4.ip_forward = 1" >> /etc/sysctl.d/k8s.conf'
	sudo -E sysctl --system

	sudo -E systemctl enable kubelet
else
	die "Kubernetes configuration not done on $ID"
fi
