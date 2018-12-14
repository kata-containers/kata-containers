#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"

setup() {
	export KUBECONFIG=/etc/kubernetes/admin.conf
	if sudo -E kubectl get runtimeclass | grep kata; then
		pod_config_dir="${BATS_TEST_DIRNAME}/runtimeclass_workloads"
	else
		pod_config_dir="${BATS_TEST_DIRNAME}/untrusted_workloads"
	fi
}

@test "Security context" {
	pod_name="security-context-test"

	# Create pod
	sudo -E kubectl create -f "${pod_config_dir}/pod-security-context.yaml"

	# Check pod creation
	sudo -E kubectl wait --for=condition=Ready pod "$pod_name"

	# Check user
	cmd="ps --user 1000 -f"
	process="tail -f /dev/null"
	sudo -E kubectl exec $pod_name -- sh -c $cmd | grep "$process"
}

teardown() {
	sudo -E kubectl delete pod "$pod_name"
}
