#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	get_pod_config_dir

	yaml_file="${pod_config_dir}/pod-security-context.yaml"
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	cmd="ps --user 1000 -f"
	exec_command=(sh -c "${cmd}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_command[@]}"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

@test "Security context" {
	pod_name="security-context-test"

	# Create pod
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check user
	process="tail -f /dev/null"
	kubectl exec $pod_name -- "${exec_command[@]}" | grep "$process"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}
