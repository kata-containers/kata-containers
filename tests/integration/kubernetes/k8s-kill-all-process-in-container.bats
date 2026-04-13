#!/usr/bin/env bats
#
# Copyright (c) 2022 AntGroup Corporation
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

	yaml_file="${pod_config_dir}/initcontainer-shareprocesspid.yaml"

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	command="ps aux || ps"

	add_exec_to_policy_settings "${policy_settings_dir}" "sh" "-c" "${command}"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

@test "Kill all processes in container" {
	# Create the pod
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod $pod_name

	# Check PID from first container
	# Capture kubectl exec output separately so a failed exec is not hidden
	# by the downstream grep.  Use 'sh -c' to ensure the shell interprets
	# $command correctly, and "[t]ail" to prevent grep itself from appearing.
	exec_output=$(kubectl exec $pod_name -c $first_container_name \
		-- sh -c "$command")
	first_pid_container=$(echo "$exec_output" | grep "[t]ail" || true)
	# Verify that the tail process didn't exist
	[ -z "$first_pid_container" ] || die "found processes pid: $first_pid_container"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
	teardown_common "${node}" "${node_start_time:-}"
}
