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
	config_name="test-configmap"
	pod_env_name="config-env-test-pod"
	pod_volume_name="configmap-volume-test-pod"

	setup_common || die "setup_common failed"
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	cmd="env"
	exec_command=(sh -c "${cmd}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_command[@]}"

	# Add policy for volume mount test
	check_config_cmd="cat /etc/config/data-1"
	check_config_exec_command=(sh -c "${check_config_cmd}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${check_config_exec_command[@]}"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"

	configmap_yaml_file="${pod_config_dir}/configmap.yaml"
	pod_yaml_file="${pod_config_dir}/pod-configmap.yaml"
	pod_volume_yaml_file="${pod_config_dir}/pod-configmap-volume.yaml"

	auto_generate_policy "${policy_settings_dir}" "${pod_yaml_file}" "${configmap_yaml_file}"
	auto_generate_policy "${policy_settings_dir}" "${pod_volume_yaml_file}" "${configmap_yaml_file}"
}

@test "ConfigMap for a pod" {
	# Create ConfigMap
	kubectl create -f "${configmap_yaml_file}"

	# View the values of the keys
	kubectl get configmaps "${config_name}" -o yaml | grep -q "data-"

	# Create a pod that consumes the ConfigMap
	kubectl create -f "${pod_yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_env_name}"

	# Check env
	grep_pod_exec_output "${pod_env_name}" "KUBE_CONFIG_1=value-1" "${exec_command[@]}"
	grep_pod_exec_output "${pod_env_name}" "KUBE_CONFIG_2=value-2" "${exec_command[@]}"
}

@test "ConfigMap propagation to volume-mounted pod" {
	original_value="value-1"
	updated_value="updated-value-1"

	# Create ConfigMap
	kubectl create -f "${configmap_yaml_file}"

	# Create a pod that consumes the ConfigMap via volume mount
	kubectl create -f "${pod_volume_yaml_file}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_volume_name}"

	# Verify initial value from volume
	grep_pod_exec_output "${pod_volume_name}" "${original_value}" "${check_config_exec_command[@]}"

	# Update ConfigMap to test propagation
	kubectl patch configmap "${config_name}" -p "{\"data\":{\"data-1\":\"${updated_value}\"}}"

	# Wait for propagation (kubelet sync period ~60s, but allow extra time for slow clusters)
	info "Waiting for ConfigMap propagation to volume-mounted pod"
	propagation_wait_time=180

	# Define check function for waitForProcess
	check_configmap_propagated() {
		pod_exec "${pod_volume_name}" "${check_config_exec_command[@]}" | grep -q "${updated_value}"
	}

	if waitForProcess "${propagation_wait_time}" "${sleep_time}" check_configmap_propagated; then
		info "ConfigMap successfully propagated to volume"
	else
		info "ConfigMap propagation test failed after ${propagation_wait_time} seconds"
		return 1
	fi
}

teardown() {
	kubectl delete pod "${pod_env_name}" --ignore-not-found=true
	kubectl delete pod "${pod_volume_name}" --ignore-not-found=true
	kubectl delete configmap "${config_name}" --ignore-not-found=true

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
	teardown_common "${node}" "${node_start_time:-}"
}
