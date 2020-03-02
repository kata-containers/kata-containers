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

RUNTIME=${RUNTIME:-kata-runtime}
RUNTIME_PATH=${RUNTIME_PATH:-$(command -v $RUNTIME)}

system_pod_wait_time=120
sleep_time=5
wait_pods_ready()
{
	# Master components provide the cluster’s control plane, including kube-apisever,
	# etcd, kube-scheduler, kube-controller-manager, etc.
	# We need to ensure their readiness before we run any container tests.
	local pods_status="kubectl get pods --all-namespaces"
	local apiserver_pod="kube-apiserver.*1/1.*Running"
	local controller_pod="kube-controller-manager.*1/1.*Running"
	local etcd_pod="etcd.*1/1.*Running"
	local scheduler_pod="kube-scheduler.*1/1.*Running"
	local dns_pod="coredns.*1/1.*Running"

	local system_pod=($apiserver_pod $controller_pod $etcd_pod $scheduler_pod $dns_pod)
	for pod_entry in "${system_pod[@]}"
	do
		waitForProcess "$system_pod_wait_time" "$sleep_time" "$pods_status | grep $pod_entry"
	done
}

cri_runtime="${CRI_RUNTIME:-crio}"
kubernetes_version=$(get_version "externals.kubernetes.version")

# store iptables if CI running on bare-metal
BAREMETAL="${BAREMETAL:-false}"
iptables_cache="${KATA_TESTS_DATADIR}/iptables_cache"
if [ "${BAREMETAL}" == true ]; then
	[ -d "${KATA_TESTS_DATADIR}" ] || sudo mkdir -p "${KATA_TESTS_DATADIR}"
	iptables-save > "$iptables_cache"
fi

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
cni_config_dir="/etc/cni/net.d"
cni_interface="cni0"
sudo rm -rf /var/lib/cni/networks/*
sudo rm -rf "${cni_config_dir}"/*
if ip a show "$cni_interface"; then
	sudo ip link set dev "$cni_interface" down
	sudo ip link del "$cni_interface"
fi

echo "Start ${cri_runtime} service"
sudo systemctl start ${cri_runtime}
max_cri_socket_check=5
wait_time_cri_socket_check=5

for i in $(seq ${max_cri_socket_check}); do
	#when the test runs two times in the CI, the second time crio takes some time to be ready
	sleep "${wait_time_cri_socket_check}"
	if [ -e "${cri_runtime_socket}" ]; then
		break
	fi

	echo "Waiting for cri socket ${cri_runtime_socket} (try ${i})"
done

sudo systemctl status "${cri_runtime}" --no-pager

echo "Init cluster using ${cri_runtime_socket}"
kubeadm_config_template="${SCRIPT_PATH}/kubeadm/config.yaml"
kubeadm_config_file="$(mktemp --tmpdir kubeadm_config.XXXXXX.yaml)"

sed -e "s|CRI_RUNTIME_SOCKET|${cri_runtime_socket}|" "${kubeadm_config_template}" > "${kubeadm_config_file}"
sed -i "s|KUBERNETES_VERSION|v${kubernetes_version/-*}|" "${kubeadm_config_file}"

trap 'sudo -E sh -c "rm -r "${kubeadm_config_file}""' EXIT

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

# default network plugin should be flannel, and its config file is taken from k8s 1.12 documentation
flannel_version="$(get_test_version "externals.flannel.version")"
flannel_url="https://raw.githubusercontent.com/coreos/flannel/${flannel_version}/Documentation/kube-flannel.yml"

arch=$("${SCRIPT_PATH}/../../.ci/kata-arch.sh")
#Load arch-specific configure file
if [ -f "${SCRIPT_PATH}/../../.ci/${arch}/kubernetes/init.sh" ]; then
        source "${SCRIPT_PATH}/../../.ci/${arch}/kubernetes/init.sh"
fi

network_plugin_config=${network_plugin_config:-$flannel_url}

kubectl apply -f "$network_plugin_config"

# we need to ensure a few specific pods ready and running
wait_pods_ready

runtimeclass_files_path="${SCRIPT_PATH}/runtimeclass_workloads"
echo "Create kata RuntimeClass resource"
kubectl create -f "${runtimeclass_files_path}/kata-runtimeclass.yaml"

# Enable the master node to be able to schedule pods.
kubectl taint nodes "$(hostname)" node-role.kubernetes.io/master:NoSchedule-
