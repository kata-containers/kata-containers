#
# Copyright (c) 2021 Red Hat, Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# This script is evoked within an OpenShift Build to product the binary image,
# which will contain the Kata Containers installation into a given destination
# directory.
#
# This contains variables and functions common to all e2e tests.

# Timeout options, mainly for use with waitForProcess(). Use them unless the
# operation needs to wait longer.
wait_time=90
sleep_time=3

# Timeout for use with `kubectl wait`, unless it needs to wait longer.
# Note: try to keep timeout and wait_time equal.
timeout=90s

# issues that can't test yet.
fc_limitations="https://github.com/kata-containers/documentation/issues/351"

# Path to the kubeconfig file which is used by kubectl and other tools.
# Note: the init script sets that variable but if you want to run the tests in
# your own provisioned cluster and you know what you are doing then you should
# overwrite it.
export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/config}"

get_pod_config_dir() {
	pod_config_dir="${BATS_TEST_DIRNAME}/runtimeclass_workloads"
	info "k8s configured to use runtimeclass"
}

# Uses crictl to pull a container image passed in $1.
# If crictl is not found then it just prints a warning.
crictl_pull() {
	local img="${1:-}"
	local cmd="crictl"
	if ! command -v "$cmd" &>/dev/null; then
		warn "$cmd not found. Cannot pull image $img"
	else
		sudo -E "$cmd" pull "$img"
	fi
}
