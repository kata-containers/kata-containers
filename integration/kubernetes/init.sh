#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail


SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../../.ci/lib.sh"
source "${SCRIPT_PATH}/../../lib/common.bash"
source "/etc/os-release" || source "/usr/lib/os-release"

cri_runtime="${CRI_RUNTIME:-crio}"
kubernetes_version=$(get_version "externals.kubernetes.version")

[ "$ID" == "fedora" ] && bash "${SCRIPT_PATH}/../../.ci/install_kubernetes.sh"

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

# Check no there are no kata processes from previous tests.
check_processes

# Remove existing CNI configurations:
sudo rm -rf /var/lib/cni/networks/*
sudo rm -rf /etc/cni/net.d/*
cni_interface="cni0"
if ip a show "$cni_interface"; then
	sudo ip link set dev "$cni_interface" down
	sudo ip link del "$cni_interface"
fi

echo "Start ${cri_runtime} service"
sudo systemctl start ${cri_runtime}

echo "Init cluster using ${cri_runtime_socket}"
kubeadm_config_template="${SCRIPT_PATH}/kubeadm/config.yaml"
kubeadm_config_file="$(mktemp --tmpdir kubeadm_config.XXXXXX.yaml)"

sed -e "s|CRI_RUNTIME_SOCKET|${cri_runtime_socket}|" "${kubeadm_config_template}" > "${kubeadm_config_file}"
sed -i "s|KUBERNETES_VERSION|v${kubernetes_version/-*}|" "${kubeadm_config_file}"

BAREMETAL="${BAREMETAL:-false}"
if [ "${BAREMETAL}" == true ] && [[ $(wc -l /proc/swaps | awk '{print $1}') -gt 1 ]]; then
	sudo swapoff -a || true
fi
sudo -E kubeadm init --config "${kubeadm_config_file}"

mkdir -p "$HOME/.kube"
sudo cp "/etc/kubernetes/admin.conf" "$HOME/.kube/config"
sudo chown $(id -u):$(id -g) "$HOME/.kube/config"
export KUBECONFIG="$HOME/.kube/config"

kubectl get nodes
kubectl get pods

arch=$("${SCRIPT_PATH}/../../.ci/kata-arch.sh")
#Load arch-specific configure file
if [ -f "${SCRIPT_PATH}/../../.ci/${arch}/kubernetes/init.sh" ]; then
        source "${SCRIPT_PATH}/../../.ci/${arch}/kubernetes/init.sh"
fi

# default network plugin should be flannel, and its config file is taken from k8s 1.12 documentation:
network_plugin_config=${network_plugin_config:-https://raw.githubusercontent.com/coreos/flannel/bc79dd1505b0c8681ece4de4c0d86c5cd2643275/Documentation/kube-flannel.yml}

kubectl apply -f "$network_plugin_config"

# The kube-dns pod usually takes around 120 seconds to get ready
# This instruction will wait until it is up and running, so we can
# start creating our containers.
dns_wait_time=120
sleep_time=5
cmd="kubectl get pods --all-namespaces | grep 'coredns.*1/1.*Running'"
waitForProcess "$dns_wait_time" "$sleep_time" "$cmd"

runtimeclass_files_path="${SCRIPT_PATH}/runtimeclass_workloads"
echo "Create kata RuntimeClass resource"
kubectl create -f "${runtimeclass_files_path}/kata-runtimeclass.yaml"

# Enable the master node to be able to schedule pods.
kubectl taint nodes "$(hostname)" node-role.kubernetes.io/master:NoSchedule-
