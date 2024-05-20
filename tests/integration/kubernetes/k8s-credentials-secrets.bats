#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "qemu-tdx" ] && skip "See: https://github.com/kata-containers/kata-containers/issues/9667"

	get_pod_config_dir
	pod_yaml_file="${pod_config_dir}/pod-secret.yaml"
	cmd="ls /tmp/secret-volume"

	# Add policy to the pod yaml file
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	exec_command="sh -c ${cmd}"
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_command}"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"

	auto_generate_policy "${policy_settings_dir}" "${pod_yaml_file}"
}

@test "Credentials using secrets" {
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
	kubectl exec $pod_name -- sh -c "$cmd" | grep -w "password"
	kubectl exec $pod_name -- sh -c "$cmd" | grep -w "username"

	# Create a pod that has access to the secret data through environment variables
	kubectl create -f "${pod_config_dir}/pod-secret-env.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$second_pod_name"

	# Display environment variables
	second_cmd="printenv"
	kubectl exec $second_pod_name -- sh -c "$second_cmd" | grep -w "SECRET_USERNAME"
	kubectl exec $second_pod_name -- sh -c "$second_cmd" | grep -w "SECRET_PASSWORD"
}

teardown() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "qemu-tdx" ] && skip "See: https://github.com/kata-containers/kata-containers/issues/9667"

	# Debugging information
	kubectl describe "pod/$pod_name"
	kubectl describe "pod/$second_pod_name"

	kubectl delete pod "$pod_name" "$second_pod_name"
	kubectl delete secret "$secret_name"

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}
