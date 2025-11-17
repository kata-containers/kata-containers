#!/usr/bin/env bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# External dependencies (not present in bare minimum busybox image):
#   - kubectl
#   - nsenter (via host_systemctl function from utils.sh)
#

function wait_till_node_is_ready() {
	local ready="False"

	while ! [[ "${ready}" == "True" ]]; do
		sleep 2s
		ready=$(kubectl get node $NODE_NAME -o jsonpath='{.status.conditions[?(@.type=="Ready")].status}')
	done
}

function restart_runtime() {
	local runtime="${1}"

	if [ "${runtime}" == "k0s-worker" ] || [ "${runtime}" == "k0s-controller" ]; then
		# do nothing, k0s will automatically load the config on the fly
		:
	elif [ "${runtime}" == "microk8s" ]; then
		host_systemctl restart snap.microk8s.daemon-containerd.service
	else
		host_systemctl daemon-reload
		host_systemctl restart "${runtime}"
	fi

	wait_till_node_is_ready
}

function restart_cri_runtime() {
	local runtime="${1}"

	if [ "${runtime}" == "k0s-worker" ] || [ "${runtime}" == "k0s-controller" ]; then
		# do nothing, k0s will automatically unload the config on the fly
		:
	elif [ "$1" == "microk8s" ]; then
		host_systemctl restart snap.microk8s.daemon-containerd.service
	else
		host_systemctl daemon-reload
		host_systemctl restart "${runtime}"
	fi
}

function cleanup_cri_runtime() {
	case $1 in
	crio)
		cleanup_crio
		;;
	containerd | k3s | k3s-agent | rke2-agent | rke2-server | k0s-controller | k0s-worker | microk8s)
		cleanup_containerd
		;;
	esac

	[ "${HELM_POST_DELETE_HOOK}" == "false" ] && return

	# Only run this code in the HELM_POST_DELETE_HOOK
	restart_cri_runtime "$1"
}

function reset_runtime() {
	kubectl label node "$NODE_NAME" katacontainers.io/kata-runtime-
	restart_cri_runtime "$1"

	if [ "$1" == "crio" ] || [ "$1" == "containerd" ]; then
		host_systemctl restart kubelet
	fi

	wait_till_node_is_ready
}

function configure_cri_runtime() {
	local runtime="${1}"

	case "${runtime}" in
	crio)
		configure_crio
		;;
	containerd | k3s | k3s-agent | rke2-agent | rke2-server | k0s-controller | k0s-worker | microk8s)
		configure_containerd "${runtime}"
		;;
	esac
}

