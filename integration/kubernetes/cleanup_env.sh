#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# This script is used to reset the kubernetes cluster

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../../lib/common.bash"

cri_runtime="${CRI_RUNTIME:-crio}"

case "${cri_runtime}" in
containerd)
	cri_runtime_socket="/run/containerd/containerd.sock"
	;;
crio)
	cri_runtime_socket="/var/run/crio/crio.sock"
	;;
*)
	echo "Runtime ${cri_runtime} not supported"
	;;
esac

export KUBECONFIG="$HOME/.kube/config"
sudo -E kubeadm reset -f --cri-socket="${cri_runtime_socket}"

sudo systemctl stop "${cri_runtime}"

sudo ip link set dev cni0 down || true
sudo ip link set dev flannel.1 down || true
sudo ip link del cni0 || true
sudo ip link del flannel.1 || true

# if CI run in bare-metal, we need a set of extra clean
BAREMETAL="${BAREMETAL:-false}"
if [ "${BAREMETAL}" == true ] && [ -f "${SCRIPT_PATH}/cleanup_bare_metal_env.sh" ]; then
	bash -f "${SCRIPT_PATH}/cleanup_bare_metal_env.sh"
fi

# Check no kata processes are left behind after reseting kubernetes
check_processes

# Checks that pods were not left
check_pods
