#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../../lib/common.bash"

export KUBECONFIG=/etc/kubernetes/admin.conf
sudo -E kubeadm reset --cri-socket=/var/run/crio/crio.sock

# Workaround to delete pods using crictl
# Needed until https://github.com/kubernetes/kubeadm/issues/748
# gets fixed
for ctr in $(sudo crictl ps --quiet); do
	sudo crictl stop "$ctr"
	sudo crictl rm "$ctr"
done
for pod in $(sudo crictl pods --quiet); do
	sudo crictl stopp "$pod"
	sudo crictl rmp "$pod"
done

sudo systemctl stop crio

sudo ip link set dev cni0 down
sudo ip link set dev flannel.1 down
sudo ip link del cni0
sudo ip link del flannel.1

# Check no processes are left behind
check_processes
