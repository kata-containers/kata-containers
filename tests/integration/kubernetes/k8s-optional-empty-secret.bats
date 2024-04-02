#!/usr/bin/env bats
#
# Copyright (c) 2021 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	get_pod_config_dir

	pod_yaml="${pod_config_dir}/pod-optional-empty-secret.yaml"
	add_allow_all_policy_to_yaml "${pod_yaml}"
}

@test "Optional and Empty Secret Volume for a pod" {
	secret_name="empty-secret"
	pod_name="optional-empty-secret-test-pod"

	# Create Empty Secret
	kubectl create secret generic "$secret_name"

	# Create a pod that consumes the "empty-secret" and "optional-missing-secret" Secrets as volumes
	kubectl create -f "${pod_yaml}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check secret folders exist
	kubectl exec $pod_name -- sh -c ls /empty-secret
	kubectl exec $pod_name -- sh -c ls /optional-missing-secret
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
	kubectl delete secret "$secret_name"
}
