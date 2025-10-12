#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"

	setup_common || die "setup_common failed"
	get_pod_config_dir
}

@test "Credentials using secrets" {
	# Add policy to pod-secret.yaml.
	pod_yaml_file="${pod_config_dir}/pod-secret.yaml"
	set_node "$pod_yaml_file" "$node"
	pod_cmd="ls /tmp/secret-volume"
	pod_exec_command=(sh -c "${pod_cmd}")
	pod_policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_exec_to_policy_settings "${pod_policy_settings_dir}" "${pod_exec_command[@]}"
	add_requests_to_policy_settings "${pod_policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${pod_policy_settings_dir}" "${pod_yaml_file}"

	# Add policy to pod-secret-env.yaml.
	pod_env_yaml_file="${pod_config_dir}/pod-secret-env.yaml"
	set_node "$pod_env_yaml_file" "$node"
	pod_env_cmd="printenv"
	pod_env_exec_command=(sh -c "${pod_env_cmd}")
	pod_env_policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_exec_to_policy_settings "${pod_env_policy_settings_dir}" "${pod_env_exec_command[@]}"
	add_requests_to_policy_settings "${pod_env_policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${pod_env_policy_settings_dir}" "${pod_env_yaml_file}" "${pod_config_dir}/inject_secret.yaml"

	secret_name="test-secret"
	pod_name="secret-test-pod"
	second_pod_name="secret-envars-test-pod"

	# Create the secret
	kubectl create -f "${pod_config_dir}/inject_secret.yaml"

	# View information about the secret
	kubectl get secret "${secret_name}" -o yaml | grep "type: Opaque"

	# Create a pod that has access to the secret through a volume
	kubectl create -f "${pod_yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# List the files
	pod_exec_with_retries "${pod_name}" "${pod_exec_command[@]}" | grep -w "password"
	pod_exec_with_retries "${pod_name}" "${pod_exec_command[@]}" | grep -w "username"

	# Create a pod that has access to the secret data through environment variables
	kubectl create -f "${pod_env_yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$second_pod_name"

	# Display environment variables
	pod_exec_with_retries "${second_pod_name}" "${pod_env_exec_command[@]}" | grep -w "SECRET_USERNAME"
	pod_exec_with_retries "${second_pod_name}" "${pod_env_exec_command[@]}" | grep -w "SECRET_PASSWORD"

	# Cleanup
	kubectl delete secret "$secret_name"
	delete_tmp_policy_settings_dir "${pod_policy_settings_dir}"
	delete_tmp_policy_settings_dir "${pod_env_policy_settings_dir}"
}

@test "Secret propagation with volume mount" {
	is_confidential_hardware || skip "Test requires shared_fs=none (confidential hardware: ${KATA_HYPERVISOR})"

	secret_name="inject-secret"
	pod_name="secret-volume-test-pod"
	check_secret_cmd="cat /tmp/secret-volume/username"
	check_secret_exec_command=(sh -c "${check_secret_cmd}")

	secret_yaml_file="${pod_config_dir}/inject_secret.yaml"
	pod_yaml_file="${pod_config_dir}/pod-secret.yaml"

	# Setup policy for volume mount test
	secret_policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	set_node "$pod_yaml_file" "$node"
	add_exec_to_policy_settings "${secret_policy_settings_dir}" "${check_secret_exec_command[@]}"
	add_requests_to_policy_settings "${secret_policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${secret_policy_settings_dir}" "${pod_yaml_file}" "${secret_yaml_file}"

	# Create Secret
	kubectl create -f "${secret_yaml_file}"
	kubectl get secret "${secret_name}" -o yaml | grep -q "username"

	# Create pod with volume mount
	kubectl create -f "${pod_yaml_file}"
	kubectl wait --for=condition=Ready --timeout=$timeout pod "${pod_name}"

	# Verify initial value
	grep_pod_exec_output "${pod_name}" "my-app" "${check_secret_exec_command[@]}"

	# Update Secret
	kubectl patch secret "${secret_name}" -p '{"stringData":{"username":"updated-username"}}'

	# Wait for propagation (kubelet sync period ~60s)
	info "Waiting for Secret propagation"
	local max_attempts=20
	local attempt=0

	while [ $attempt -lt $max_attempts ]; do
		if pod_exec_with_retries "${pod_name}" "${check_secret_exec_command[@]}" | grep -q "updated-username"; then
			info "Secret successfully propagated"
			break
		fi
		info "Attempt $((attempt + 1))/$max_attempts: waiting..."
		sleep 5
		attempt=$((attempt + 1))
	done

	# Verify propagation succeeded
	grep_pod_exec_output "${pod_name}" "updated-username" "${check_secret_exec_command[@]}"

	# Cleanup for this test
	kubectl delete pod "${pod_name}" --ignore-not-found=true
	kubectl delete secret "${secret_name}" --ignore-not-found=true
	delete_tmp_policy_settings_dir "${secret_policy_settings_dir}"
}

teardown() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"

	teardown_common "${node}" "${node_start_time:-}"
}
