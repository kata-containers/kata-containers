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

	# Add policy to the pod yaml
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	command1=(sh -c ls /empty-secret)
	add_exec_to_policy_settings "${policy_settings_dir}" "${command1[@]}"

	command2=(sh -c ls /optional-missing-secret)
	add_exec_to_policy_settings "${policy_settings_dir}" "${command2[@]}"

	auto_generate_policy "${policy_settings_dir}" "${pod_yaml}"
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
	kubectl exec $pod_name -- "${command1[@]}"
	kubectl exec $pod_name -- "${command2[@]}"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
	kubectl delete secret "$secret_name"

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}
