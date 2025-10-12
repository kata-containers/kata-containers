#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	setup_common || die "setup_common failed"
	get_pod_config_dir
}

@test "ConfigMap for a pod" {
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	cmd="env"
	exec_command=(sh -c "${cmd}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_command[@]}"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"

	configmap_yaml_file="${pod_config_dir}/configmap.yaml"
	pod_yaml_file="${pod_config_dir}/pod-configmap.yaml"

	auto_generate_policy "${policy_settings_dir}" "${pod_yaml_file}" "${configmap_yaml_file}"

	config_name="test-configmap"
	pod_name="config-env-test-pod"

	# Create ConfigMap
	kubectl create -f "${configmap_yaml_file}"

	# View the values of the keys
	kubectl get configmaps $config_name -o yaml | grep -q "data-"

	# Create a pod that consumes the ConfigMap
	kubectl create -f "${pod_yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check env
	grep_pod_exec_output "${pod_name}" "KUBE_CONFIG_1=value-1" "${exec_command[@]}"
	grep_pod_exec_output "${pod_name}" "KUBE_CONFIG_2=value-2" "${exec_command[@]}"

	# Cleanup
	kubectl delete pod "$pod_name"
	kubectl delete configmap "$config_name"
	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}

@test "ConfigMap propagation in pod with sharedfs=none" {
	if ! is_confidential_hardware; then
		skip "Test requires shared_fs=none (confidential hardware: ${KATA_HYPERVISOR})"
	fi

	configmap_name="test-configmap-propagation"
	pod_name="configmap-volume-test-pod"
	check_config_cmd="cat /etc/config/data-1"
	check_config_exec_command=(sh -c "${check_config_cmd}")

	configmap_yaml_file="${pod_config_dir}/configmap.yaml"
	pod_yaml_file="${pod_config_dir}/pod-configmap-volume.yaml"

	# Setup policy
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_exec_to_policy_settings "${policy_settings_dir}" "${check_config_exec_command[@]}"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${pod_yaml_file}" "${configmap_yaml_file}"

	# Create ConfigMap
	kubectl create -f "${configmap_yaml_file}"
	kubectl get configmap "${configmap_name}" -o yaml | grep -q "data-1"

	# Create pod
	kubectl create -f "${pod_yaml_file}"
	kubectl wait --for=condition=Ready --timeout=$timeout pod "${pod_name}"

	# Verify initial value
	grep_pod_exec_output "${pod_name}" "value-1" "${check_config_exec_command[@]}"

	# Update ConfigMap
	kubectl patch configmap "${configmap_name}" -p '{"data":{"data-1":"updated-value-1"}}'

	# Wait for propagation (kubelet sync period ~60s)
	info "Waiting for ConfigMap propagation"
	local max_attempts=20
	local attempt=0

	while [ $attempt -lt $max_attempts ]; do
		if pod_exec_with_retries "${pod_name}" "${check_config_exec_command[@]}" | grep -q "updated-value-1"; then
			info "ConfigMap successfully propagated"
			# Cleanup
			kubectl delete pod "${pod_name}" --ignore-not-found=true
			kubectl delete configmap "${configmap_name}" --ignore-not-found=true
			delete_tmp_policy_settings_dir "${policy_settings_dir}"
			return 0
		fi
		info "Attempt $((attempt + 1))/$max_attempts: waiting..."
		sleep 5
		attempt=$((attempt + 1))
	done

	# Cleanup on failure
	kubectl describe "pod/${pod_name}" || true
	kubectl delete pod "${pod_name}" --ignore-not-found=true
	kubectl delete configmap "${configmap_name}" --ignore-not-found=true
	delete_tmp_policy_settings_dir "${policy_settings_dir}"

	# If we get here, propagation failed
	return 1
}

teardown() {
	teardown_common "${node}" "${node_start_time:-}"
}
