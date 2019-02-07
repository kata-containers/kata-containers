#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"

setup() {
	export KUBECONFIG="$HOME/.kube/config"
	if kubectl get runtimeclass | grep kata; then
		pod_config_dir="${BATS_TEST_DIRNAME}/runtimeclass_workloads"
	else
		pod_config_dir="${BATS_TEST_DIRNAME}/untrusted_workloads"
	fi
}

@test "Security context" {
	pod_name="security-context-test"

	# Create pod
	kubectl create -f "${pod_config_dir}/pod-security-context.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready pod "$pod_name"

	# Check user
	cmd="ps --user 1000 -f"
	process="tail -f /dev/null"
	kubectl exec $pod_name -- sh -c $cmd | grep "$process"
}

teardown() {
	kubectl delete pod "$pod_name"
}
