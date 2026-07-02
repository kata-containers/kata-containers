#!/usr/bin/env bats
#
# Copyright (c) 2026 Schwarz Digits Cloud GmbH & Co. KG
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	setup_common || die "setup_common failed"
}

# Wait for a pod to reach a terminated state (Completed or Error).
# Copyright (c) 2026 NVIDIA Corporation
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

@test "On pod termination, SIGTERM is sent only once" {
	pod_name="graceful-termination-test"
	yaml_file="${pod_config_dir}/pod-graceful-termination.yaml"
	# Add policy to the yaml file
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_requests_to_policy_settings "${policy_settings_dir}" "GetDiagnosticDataRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"

	# Create pod
	kubectl create -f "${yaml_file}"

	# Add finalizer so the pod is not fully removed during the test
	kubectl patch pod "$pod_name" --patch '{"metadata":{"finalizers":["katacontainers.io/test-finalizer"]}}' --type=merge

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

    # Send a SIGTERM by deleting the pod
	kubectl delete pod "$pod_name" --wait=false

	# Wait for pod deletion
	wait_for_pod_terminated "${pod_name}"

	local reason
	reason=$(kubectl get pod "${pod_name}" \
		-o jsonpath='{.status.containerStatuses[0].state.terminated.reason}')
	[ "${reason}" = "Error" ] # Error since we rely on terminationGracePeriodSeconds to terminate the Pod

	local message
	message=$(kubectl get pod "${pod_name}" \
		-o jsonpath='{.status.containerStatuses[0].state.terminated.message}')
	echo "termination message: ${message}"
	[ "${message}" = "start, received SIGTERM" ] # Must be this exact message, with only one SIGTERM

	# Remove the finalizer to let kubernetes fully delete the pod
	kubectl patch pod "$pod_name" --patch '{"metadata":{"finalizers":null}}' --type=merge
}

teardown() {
    kubectl patch pod "$pod_name" '{"metadata":{"finalizers":null}}' --type=merge || true
	kubectl delete pod "$pod_name"
	delete_tmp_policy_settings_dir "${policy_settings_dir}"
	teardown_common "${node}" "${node_start_time:-}"
}
