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

k8s_create_pod_ready() {
	local pod_name="$1"
	local pod_yaml="$2"

	local wait_time=300
	local max_attempts=5
	local attempt_num

	for attempt_num in $(seq 1 "${max_attempts}"); do
		# First,forcefully deleting resources
		kubectl delete -f "${pod_yaml}" --ignore-not-found=true --now --timeout=$wait_time

		kubectl create -f "${pod_yaml}"
		if [ $? -ne 0 ]; then
			# Failed to create Pod.Aborting test.
			continue
		fi

		# Check pod creation
		kubectl wait --for=condition=Ready --timeout=$wait_time pod "$pod_name"
		if [ "$status" -eq 0 ]; then
			# Test Succeeded on attempt #${attempt_num}
			return 0
		fi

		# Retry
		if [ "${attempt_num}" -lt "${max_attempts}" ]; then
			info "Waiting for 5 seconds before next attempt..."
			sleep 5
		fi
	done

	#Test Failed after ${max_attempts} attempts.
	return 1
}

@test "Optional and Empty ConfigMap Volume for a pod" {
	config_name="empty-config"
	pod_name="optional-empty-config-test-pod"

	# Create Empty ConfigMap
	kubectl create configmap "$config_name"

	# Create a pod that consumes the "empty-config" and "optional-missing-config" ConfigMaps as volumes
	# kubectl create -f "${pod_yaml}"
	# Check pod creation
	# kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"
	# Retry for ready pod
	k8s_create_pod_ready "$pod_name" "${pod_yaml}"

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
