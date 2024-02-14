#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	pod_name="busybox"
	first_container_name="first-test-container"
	second_container_name="second-test-container"

	get_pod_config_dir

	test_yaml_file="${pod_config_dir}/pid-ns-busybox-pod.yaml"
	cp "$pod_config_dir/busybox-pod.yaml" "${test_yaml_file}"

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	ps_command="ps"
	add_exec_to_policy_settings "${policy_settings_dir}" "${ps_command}"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${test_yaml_file}"
}

@test "Check PID namespaces" {
	# Create the pod
	kubectl create -f "${test_yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod $pod_name

	# Check PID from first container
	first_pid_container=$(kubectl exec $pod_name -c $first_container_name \
		-- $ps_command | grep "/pause")
	# Verify that is not empty
	check_first_pid=$(echo $first_pid_container | wc -l)
	[ "$check_first_pid" == "1" ]

	# Check PID from second container
	second_pid_container=$(kubectl exec $pod_name -c $second_container_name \
		-- $ps_command | grep "/pause")
	# Verify that is not empty
	check_second_pid=$(echo $second_pid_container | wc -l)
	[ "$check_second_pid" == "1" ]

	[ "$first_pid_container" == "$second_pid_container" ]
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"

	rm "${test_yaml_file}"
	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}
