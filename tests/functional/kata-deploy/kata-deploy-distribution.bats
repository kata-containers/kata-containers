#!/usr/bin/env bats
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Helm template tests for k8sDistribution reaching the install. No cluster
# required.
#
# The value picks which host directory is mounted at /etc/containerd, while the
# install detects the node's CRI runtime itself and picks the file to write inside
# that mount. The install can only refuse a disagreement between the two if it is
# told what was declared, so these tests cover that it is.

load "${BATS_TEST_DIRNAME}/../../common.bash"

source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

CHART_PATH="$(get_chart_path)"

render() {
	helm template kata-deploy "${CHART_PATH}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		"$@"
}

@test "Helm template: the declared distribution reaches the install, in both modes" {
	local mode rendered
	for mode in job daemonset; do
		rendered=$(render --set "deploymentMode=${mode}" --set k8sDistribution=k3s)

		echo "${rendered}" | grep -q 'name: K8S_DISTRIBUTION'
		echo "${rendered}" | grep -A1 'name: K8S_DISTRIBUTION' | grep -q 'value: "k3s"'
	done
}

@test "Helm template: pinning the containerd directory takes the check out of the way" {
	# containerd.configDir overrides the very derivation the install would be
	# checking, so an operator who set it is taken at their word. The flavour is
	# still declared: it decides the kubelet root directory too, which a pinned
	# containerd directory says nothing about.
	local rendered
	rendered=$(render --set deploymentMode=job \
		--set k8sDistribution=k0s \
		--set 'containerd.configDir=/etc/my-containerd/')

	echo "${rendered}" | grep -A1 'name: K8S_DISTRIBUTION' | grep -q 'value: "k0s"'
	echo "${rendered}" | grep -A1 'name: CONTAINERD_CONFIG_DIR' | grep -q 'value: "/etc/my-containerd/"'
}

@test "Helm template: the containerd directory is only reported when pinned" {
	local rendered
	rendered=$(render --set deploymentMode=job --set k8sDistribution=k0s)

	if echo "${rendered}" | grep -q 'name: CONTAINERD_CONFIG_DIR'; then
		echo "CONTAINERD_CONFIG_DIR was passed without an explicit containerd.configDir" >&2
		return 1
	fi
}
