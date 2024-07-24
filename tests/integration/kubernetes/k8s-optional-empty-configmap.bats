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

	pod_yaml="${pod_config_dir}/pod-optional-empty-configmap.yaml"
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	empty_command="ls /empty-config"
	exec_empty_command=(sh -c "${empty_command}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_empty_command[@]}"

	optional_command="ls /optional-missing-config"
	exec_optional_command=(sh -c "${optional_command}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_optional_command[@]}"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${pod_yaml}"
}

@test "Optional and Empty ConfigMap Volume for a pod" {
	config_name="empty-config"
	pod_name="optional-empty-config-test-pod"

	# Create Empty ConfigMap
	kubectl create configmap "$config_name"

	# Create a pod that consumes the "empty-config" and "optional-missing-config" ConfigMaps as volumes
	kubectl create -f "${pod_yaml}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check configmap folders exist
	kubectl exec $pod_name -- "${exec_empty_command[@]}"
	kubectl exec $pod_name -- "${exec_optional_command[@]}"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
	kubectl delete configmap "$config_name"

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}
