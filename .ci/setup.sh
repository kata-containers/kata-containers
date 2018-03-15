#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source /etc/os-release
source "${cidir}/lib.sh"

apply_depends_on

arch=$(arch)

echo "Set up environment"
if [ "$ID" == ubuntu ];then
	bash -f "${cidir}/setup_env_ubuntu.sh"
elif [ "$ID" == fedora ];then
	bash -f "${cidir}/setup_env_fedora.sh"
else
	die "ERROR: Unrecognised distribution."
	exit 1
fi

if [ "$arch" = x86_64 ]; then
	if grep -q "N" /sys/module/kvm_intel/parameters/nested; then
		echo "enable Nested Virtualization"
		sudo modprobe -r kvm_intel
		sudo modprobe kvm_intel nested=1
	fi
else
	die "Unsupported architecture: $arch"
fi

echo "Install shim"
bash -f ${cidir}/install_shim.sh

echo "Install proxy"
bash -f ${cidir}/install_proxy.sh

echo "Install runtime"
bash -f ${cidir}/install_runtime.sh

echo "Install CNI plugins"
bash -f ${cidir}/install_cni_plugins.sh

echo "Install CRI-O"
bash -f ${cidir}/install_crio.sh

echo "Drop caches"
sync
sudo -E PATH=$PATH bash -c "echo 3 > /proc/sys/vm/drop_caches"
