#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	setup_common || die "setup_common failed"
	pod_name="busybox"
	first_container_name="first-test-container"
	second_container_name="second-test-container"

	test_yaml_file="${BATS_TEST_TMPDIR}/pid-ns-busybox-pod.yaml"
	cp "$pod_config_dir/busybox-pod.yaml" "${test_yaml_file}"

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	pidns_command=(readlink /proc/self/ns/pid)
	add_exec_to_policy_settings "${policy_settings_dir}" "${pidns_command[@]}"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
}

@test "Check PID namespaces" {
	auto_generate_policy "${policy_settings_dir}" "${test_yaml_file}"
	# Create the pod
	kubectl create -f "${test_yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod $pod_name

	# Compare the namespaces themselves: Sandbox API runtimes need not create
	# a /pause container. Strip \r because tty containers return CRLF.
	first_pid_container=$(kubectl exec "$pod_name" -c "$first_container_name" \
		-- "${pidns_command[@]}" | tr -d '\r')
	[[ "$first_pid_container" =~ ^pid:\[[0-9]+\]$ ]]

	second_pid_container=$(kubectl exec "$pod_name" -c "$second_container_name" \
		-- "${pidns_command[@]}" | tr -d '\r')
	[[ "$second_pid_container" =~ ^pid:\[[0-9]+\]$ ]]

	[ "$first_pid_container" == "$second_pid_container" ]
}

@test "PID namespaces are isolated when process sharing is disabled" {
	yq -i '.spec.shareProcessNamespace = false' "${test_yaml_file}"
	auto_generate_policy "${policy_settings_dir}" "${test_yaml_file}"
	kubectl create -f "${test_yaml_file}"
	kubectl wait --for=condition=Ready --timeout="$timeout" pod "$pod_name"

	first_pid_container=$(kubectl exec "$pod_name" -c "$first_container_name" \
		-- "${pidns_command[@]}" | tr -d '\r')
	[[ "$first_pid_container" =~ ^pid:\[[0-9]+\]$ ]]
	second_pid_container=$(kubectl exec "$pod_name" -c "$second_container_name" \
		-- "${pidns_command[@]}" | tr -d '\r')
	[[ "$second_pid_container" =~ ^pid:\[[0-9]+\]$ ]]

	[ "$first_pid_container" != "$second_pid_container" ]
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"

	rm "${test_yaml_file}"
	delete_tmp_policy_settings_dir "${policy_settings_dir}"
	teardown_common "${node}" "${node_start_time:-}"
}
