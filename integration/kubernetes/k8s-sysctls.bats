#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"
load "${BATS_TEST_DIRNAME}/../../lib/common.bash"

setup() {
	export KUBECONFIG="$HOME/.kube/config"
	pod_name="sysctl-test"
	get_pod_config_dir
}

@test "Setting sysctl" {
	# Create pod
	kubectl apply -f "${pod_config_dir}/pod-sysctl.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready pod "$pod_name"

	# Check sysctl configuration
	cmd="cat /proc/sys/kernel/shm_rmid_forced"
	result=$(kubectl exec $pod_name -- sh -c "$cmd")
	[ "${result}" = 0 ]
}

teardown() {
	kubectl delete pod "$pod_name"
}
