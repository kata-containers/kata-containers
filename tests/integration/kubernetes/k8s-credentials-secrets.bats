#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"

	secret_name="test-secret"
	pod_name="secret-test-pod"
	second_pod_name="secret-envars-test-pod"

	setup_common || die "setup_common failed"

	# Add policy to pod-secret.yaml.
	pod_yaml_file="${pod_config_dir}/pod-secret.yaml"
	set_node "${pod_yaml_file}" "${node}"
	pod_cmd="ls /tmp/secret-volume"
	pod_exec_command=(sh -c "${pod_cmd}")

	# Also add policy for reading secret content (for propagation test)
	check_secret_cmd="cat /tmp/secret-volume/username"
	check_secret_exec_command=(sh -c "${check_secret_cmd}")

	pod_policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_exec_to_policy_settings "${pod_policy_settings_dir}" "${pod_exec_command[@]}"
	add_exec_to_policy_settings "${pod_policy_settings_dir}" "${check_secret_exec_command[@]}"
	add_requests_to_policy_settings "${pod_policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${pod_policy_settings_dir}" "${pod_yaml_file}" "${pod_config_dir}/inject_secret.yaml"

	# Add policy to pod-secret-env.yaml.
	pod_env_yaml_file="${pod_config_dir}/pod-secret-env.yaml"
	set_node "${pod_env_yaml_file}" "${node}"
	pod_env_cmd="printenv"
	pod_env_exec_command=(sh -c "${pod_env_cmd}")
	pod_env_policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_exec_to_policy_settings "${pod_env_policy_settings_dir}" "${pod_env_exec_command[@]}"
	add_requests_to_policy_settings "${pod_env_policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${pod_env_policy_settings_dir}" "${pod_env_yaml_file}" "${pod_config_dir}/inject_secret.yaml"
}

@test "Credentials using secrets" {
	# Create the secret
	kubectl create -f "${pod_config_dir}/inject_secret.yaml"

	# View information about the secret
	kubectl get secret "${secret_name}" -o yaml | grep "type: Opaque"

	# Create a pod that has access to the secret through a volume
	kubectl create -f "${pod_yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_name}"

	# List the files
	pod_exec "${pod_name}" "${pod_exec_command[@]}" | grep -w "password"
	pod_exec "${pod_name}" "${pod_exec_command[@]}" | grep -w "username"

	# Create a pod that has access to the secret data through environment variables
	kubectl create -f "${pod_env_yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${second_pod_name}"

	# Display environment variables
	pod_exec "${second_pod_name}" "${pod_env_exec_command[@]}" | grep -w "SECRET_USERNAME"
	pod_exec "${second_pod_name}" "${pod_env_exec_command[@]}" | grep -w "SECRET_PASSWORD"
}

@test "Secret propagation to volume-mounted pod" {
	original_username="my-app"
	updated_username="updated-username"

	# Create the secret
	kubectl create -f "${pod_config_dir}/inject_secret.yaml"

	# Create a pod that has access to the secret through a volume
	kubectl create -f "${pod_yaml_file}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_name}"

	# Verify initial secret value
	grep_pod_exec_output "${pod_name}" "${original_username}" "${check_secret_exec_command[@]}"

	# Update Secret to test propagation
	kubectl patch secret "${secret_name}" -p "{\"stringData\":{\"username\":\"${updated_username}\"}}"

	# Wait for propagation (kubelet sync period ~60s, but allow extra time for slow clusters)
	info "Waiting for Secret propagation to volume-mounted pod"
	propagation_wait_time=180

	# Define check function for waitForProcess
	check_secret_propagated() {
		pod_exec "${pod_name}" "${check_secret_exec_command[@]}" | grep -q "${updated_username}"
	}

	if waitForProcess "${propagation_wait_time}" "${sleep_time}" check_secret_propagated; then
		info "Secret successfully propagated to volume"
	else
		info "Secret propagation test failed after ${propagation_wait_time} seconds"
		return 1
	fi
}

teardown() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"

	kubectl delete pod "${pod_name}" --ignore-not-found=true
	kubectl delete pod "${second_pod_name}" --ignore-not-found=true
	kubectl delete secret "${secret_name}" --ignore-not-found=true

	delete_tmp_policy_settings_dir "${pod_policy_settings_dir}"
	delete_tmp_policy_settings_dir "${pod_env_policy_settings_dir}"

	teardown_common "${node}" "${node_start_time:-}"
}
