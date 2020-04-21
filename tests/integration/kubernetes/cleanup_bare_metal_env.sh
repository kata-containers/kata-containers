#!/bin/bash
#
# Copyright (c) 2019 ARM Limited
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../../lib/common.bash"

iptables_cache="${KATA_TESTS_DATADIR}/iptables_cache"

# The kubeadm reset process does not reset or clean up iptables rules
# you must do it manually
# Here, we restore the iptables based on the previously cached file.
sudo iptables-restore < "$iptables_cache"

# The kubeadm reset process does not clean your kubeconfig files.
# you must remove them manually.
sudo -E rm -rf "$HOME/.kube"

# Remove existing CNI configurations and binaries.
sudo sh -c 'rm -rf /var/lib/cni/networks/*'
sudo sh -c 'rm -rf /opt/cni/bin/*'

# delete containers resource created by runc
cri_runtime="${CRI_RUNTIME:-crio}"
case "${cri_runtime}" in
containerd)
        readonly runc_path=$(command -v runc)
        ;;
crio)
        readonly runc_path="/usr/local/bin/crio-runc"
        ;;
*)
        echo "Runtime ${cri_runtime} not supported"
	exit 0
        ;;
esac

runc_container_union="$($runc_path list)"
if [ -n "$runc_container_union" ]; then
	while IFS='$\n' read runc_container; do
		container_id="$(echo "$runc_container" | awk '{print $1}')"
		if [ "$container_id" != "ID" ]; then
			$runc_path delete -f $container_id
		fi
	done <<< "${runc_container_union}"
fi

# when pipeline consists of grep, it may fail unnecessarily
# when no line selected.
veth_interfaces_union=$(set +o pipefail; sudo ip link | grep "veth" | awk '{print $2}' | cut -d '@' -f1)

# delete stale veth interfaces, which is named after vethXXX.
if [ -n "$veth_interfaces_union" ]; then
	while read veth_interface; do
		sudo ip link set dev $veth_interface down
		sudo ip link del $veth_interface
	done <<< "$veth_interfaces_union"
fi
