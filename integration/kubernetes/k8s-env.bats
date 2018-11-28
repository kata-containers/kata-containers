#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"

setup() {
	export KUBECONFIG=/etc/kubernetes/admin.conf
	pod_name="test-env"
	pod_config_dir="${BATS_TEST_DIRNAME}/untrusted_workloads"
}

@test "Environment variables" {
	# Create pod
	sudo -E kubectl create -f "${pod_config_dir}/pod-env.yaml"

	# Check pod creation
	sudo -E kubectl wait --for=condition=Ready pod "$pod_name"

	# Print environment variables
	cmd="printenv"
	sudo -E kubectl exec $pod_name -- sh -c $cmd | grep "MY_POD_NAME=$pod_name"
}

teardown() {
	sudo -E kubectl delete pod "$pod_name"
}
