#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {

	pod_name="sysctl-test"
	get_pod_config_dir

	yaml_file="${pod_config_dir}/pod-sysctl.yaml"

	# Add policy to yaml
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	cmd="cat /proc/sys/kernel/shm_rmid_forced"
	exec_command=(sh -c "${cmd}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_command[@]}"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

@test "Setting sysctl" {
	# Create pod
	kubectl apply -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod $pod_name

	# Check sysctl configuration
	result=$(kubectl exec $pod_name -- "${exec_command[@]}")
	[ "${result}" = 0 ]
}

teardown() {

	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}
