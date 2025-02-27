#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	pod_name="test-env"
	get_pod_config_dir

	yaml_file="${pod_config_dir}/pod-env.yaml"
	cmd="printenv"

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	exec_command=(sh -c "${cmd}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_command[@]}"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

@test "Environment variables" {
	# Create pod
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Print environment variables
	kubectl exec $pod_name -- "${exec_command[@]}" | grep "MY_POD_NAME=$pod_name"
	kubectl exec $pod_name -- "${exec_command[@]}" | \
		grep "HOST_IP=\([0-9]\+\(\.\|$\)\)\{4\}"
	# Requested 32Mi of memory
	kubectl exec $pod_name -- "${exec_command[@]}" | \
		grep "MEMORY_REQUESTS=$((1024 * 1024 * 32))"
	# Memory limits allocated by the node
	kubectl exec $pod_name -- "${exec_command[@]}" | grep "MEMORY_LIMITS=[1-9]\+"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}
