#!/usr/bin/env bats
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Test termination log propagation via GetDiagnosticData RPC.
# These tests target shared_fs=none configurations (e.g. qemu-coco-dev)
# where the host cannot directly read guest files.

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"

setup() {
	# These tests only make sense on CoCo platforms (shared_fs=none).
	if ! is_confidential_runtime_class; then
		skip "Test requires a CoCo runtime class (shared_fs=none)"
	fi

	setup_common || die "setup_common failed"
}

# Wait for a pod to reach a terminated state (Completed or Error).
wait_for_pod_terminated() {
	local pod_name="$1"
	local elapsed=0
	local poll=5

	while [ "${elapsed}" -lt "${wait_time}" ]; do
		local reason
		reason=$(kubectl get pod "${pod_name}" \
			-o jsonpath='{.status.containerStatuses[0].state.terminated.reason}' 2>/dev/null || true)
		if [[ "${reason}" == "Completed" ]] || [[ "${reason}" == "Error" ]]; then
			return 0
		fi
		sleep "${poll}"
		elapsed=$((elapsed + poll))
	done
	echo "Pod ${pod_name} did not terminate within ${wait_time}s" >&2
	return 1
}

@test "Termination log: successful exit with policy allowing GetDiagnosticDataRequest" {
	pod_name="pod-termination-log-success"
	yaml_file="${pod_config_dir}/${pod_name}.yaml"

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_requests_to_policy_settings "${policy_settings_dir}" "GetDiagnosticDataRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"

	kubectl create -f "${yaml_file}"
	wait_for_pod_terminated "${pod_name}"

	local reason
	reason=$(kubectl get pod "${pod_name}" \
		-o jsonpath='{.status.containerStatuses[0].state.terminated.reason}')
	[ "${reason}" = "Completed" ]

	local message
	message=$(kubectl get pod "${pod_name}" \
		-o jsonpath='{.status.containerStatuses[0].state.terminated.message}')
	echo "termination message: ${message}"
	[[ "${message}" == *"successful exit message"* ]]

	kubectl delete pod "${pod_name}"
	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}

@test "Termination log: failed exit with policy allowing GetDiagnosticDataRequest" {
	pod_name="pod-termination-log-fail"
	yaml_file="${pod_config_dir}/${pod_name}.yaml"

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_requests_to_policy_settings "${policy_settings_dir}" "GetDiagnosticDataRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"

	kubectl create -f "${yaml_file}"
	wait_for_pod_terminated "${pod_name}"

	local reason
	reason=$(kubectl get pod "${pod_name}" \
		-o jsonpath='{.status.containerStatuses[0].state.terminated.reason}')
	[ "${reason}" = "Error" ]

	local message
	message=$(kubectl get pod "${pod_name}" \
		-o jsonpath='{.status.containerStatuses[0].state.terminated.message}')
	echo "termination message: ${message}"
	[[ "${message}" == *"failure exit message"* ]]

	kubectl delete pod "${pod_name}"
	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}

@test "Termination log: request blocked by default CoCo policy" {
	if ! auto_generate_policy_enabled; then
		echo "# Skipping default CoCo policy check: requires AUTO_GENERATE_POLICY=yes" >&3
		return 0
	fi

	pod_name="pod-termination-log-success"
	yaml_file="${pod_config_dir}/${pod_name}.yaml"

	# Generate policy with default settings — GetDiagnosticDataRequest is denied.
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"

	kubectl create -f "${yaml_file}"
	wait_for_pod_terminated "${pod_name}"

	# The container should still stop cleanly (best-effort).
	local reason
	reason=$(kubectl get pod "${pod_name}" \
		-o jsonpath='{.status.containerStatuses[0].state.terminated.reason}')
	[ "${reason}" = "Completed" ]

	# The termination message should be empty because the RPC was denied by policy.
	local message
	message=$(kubectl get pod "${pod_name}" \
		-o jsonpath='{.status.containerStatuses[0].state.terminated.message}')
	echo "termination message (expected empty): '${message}'"
	[ -z "${message}" ]

	kubectl delete pod "${pod_name}"
	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}

teardown() {
	if ! is_confidential_runtime_class; then
		return
	fi

	# Debugging information
	kubectl describe "pod/${pod_name:-}" || true
	kubectl get "pod/${pod_name:-}" -o yaml || true

	kubectl delete pod "${pod_name:-}" --ignore-not-found=true
	if [ -n "${policy_settings_dir:-}" ]; then
		delete_tmp_policy_settings_dir "${policy_settings_dir}"
	fi
	teardown_common "${node}" "${node_start_time:-}"
}
