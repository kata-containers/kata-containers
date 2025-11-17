#!/usr/bin/env bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# External dependencies (not present in bare minimum busybox image):
#   - kubectl
#   - nsenter (via host_systemctl function from utils.sh)
#

function get_container_runtime() {

	local runtime=$(kubectl get node $NODE_NAME -o jsonpath='{.status.nodeInfo.containerRuntimeVersion}')
	local microk8s=$(kubectl get node $NODE_NAME -o jsonpath='{.metadata.labels.microk8s\.io\/cluster}')
	if [ "$?" -ne 0 ]; then
                die "invalid node name"
	fi

	if echo "$runtime" | grep -qE "cri-o"; then
		echo "cri-o"
	elif [ "$microk8s" == "true" ]; then
		echo "microk8s"
	elif echo "$runtime" | grep -qE 'containerd.*-k3s'; then
		if host_systemctl is-active --quiet rke2-agent; then
			echo "rke2-agent"
		elif host_systemctl is-active --quiet rke2-server; then
			echo "rke2-server"
		elif host_systemctl is-active --quiet k3s-agent; then
			echo "k3s-agent"
		else
			echo "k3s"
		fi
	# Note: we assumed you used a conventional k0s setup and k0s will generate a systemd entry k0scontroller.service and k0sworker.service respectively
	# and it is impossible to run this script without a kubelet, so this k0s controller must also have worker mode enabled
	elif host_systemctl is-active --quiet k0scontroller; then
		echo "k0s-controller"
	elif host_systemctl is-active --quiet k0sworker; then
		echo "k0s-worker"
	else
		echo "$runtime" | awk -F '[:]' '{print $1}'
	fi
}

function is_containerd_capable_of_using_drop_in_files() {
	local runtime="$1"

	if [ "$runtime" == "crio" ]; then
		# This should never happen but better be safe than sorry
		echo "false"
		return
	fi

	if [[ "$runtime" =~ ^(k0s-worker|k0s-controller)$ ]]; then
		# k0s does the work of using drop-in files better than any other "k8s distro", so
		# we don't mess up with what's being correctly done.
		echo "false"
		return
	fi

	if [ "$runtime" == "microk8s" ]; then
		# microk8s use snap containerd
		echo "false"
		return
	fi

	local version_major=$(kubectl get node $NODE_NAME -o jsonpath='{.status.nodeInfo.containerRuntimeVersion}' | grep -oE '[0-9]+\.[0-9]+' | cut -d'.' -f1)
	if [ $version_major -lt 2 ]; then
		# Only containerd 2.0 does the merge of the plugins section from different snippets,
		# instead of overwritting the whole section, which makes things considerably more
		# complicated for us to deal with.
		#
		# It's been discussed with containerd community, and the patch needed will **NOT** be
		# backported to the release 1.7, as that breaks the behaviour from an existing release.
		echo "false"
		return
	fi

	echo "true"
}

