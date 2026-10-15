#!/usr/bin/env bats
#
# Copyright (c) 2025 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	setup_common || die "setup_common failed"

	pod_name="privileged"
	yaml_file="${pod_config_dir}/pod-privileged.yaml"
	nesting_pod_name="cgroup-v2-nesting"
	nesting_yaml_file="${pod_config_dir}/pod-cgroup-v2-nesting.yaml"

	cmd_nsenter=(nsenter --mount=/proc/1/ns/mnt true)
	cmd_nested_cgroup=(sh -c 'test "$(cut -d: -f3 /proc/self/cgroup)" = "/kata-exec-test"')

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_exec_to_policy_settings "${policy_settings_dir}" "${cmd_nsenter[@]}"
	add_exec_to_policy_settings "${policy_settings_dir}" "${cmd_nested_cgroup[@]}"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
	auto_generate_policy "${policy_settings_dir}" "${nesting_yaml_file}"
}

# This should succeed because the CI uses kata-deploy which sets
# privileged_without_host_devices to true.
@test "Privileged pod runs and is able to execute privileged operations" {
	kubectl apply -f "${yaml_file}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_name}"
	kubectl exec "${pod_name}" -- "${cmd_nsenter[@]}"
}

@test "Exec into a cgroup v2 nested container" {
	kubectl apply -f "${nesting_yaml_file}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${nesting_pod_name}"

	waitForProcess "${wait_time}" "${sleep_time}" \
		"kubectl logs ${nesting_pod_name} | grep -E '^READY$|SKIP: not cgroup v2'"

	logs=$(kubectl logs "${nesting_pod_name}")
	if echo "${logs}" | grep -q "SKIP: not cgroup v2"; then
		skip "guest is not using cgroup v2"
	fi

	# The exec process must join PID 1's leaf, not the inner parent node.
	kubectl exec "${nesting_pod_name}" -- "${cmd_nested_cgroup[@]}"
}

teardown() {
	echo "Pod logs:"
	kubectl logs "${pod_name}" || true
	kubectl logs "${nesting_pod_name}" || true

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
	teardown_common "${node}" "${node_start_time:-}"
}
