#!/usr/bin/env bats
#
# Copyright (c) 2021 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"
	
	get_pod_config_dir

	pod_name="nested-configmap-secret-pod"
	yaml_file="${pod_config_dir}/pod-nested-configmap-secret.yaml"

	# Add policy to yaml file
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	exec_command1=(cat /config/config_key)
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_command1[@]}"

	exec_command2=(cat /config/secret/secret_key)
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_command2[@]}"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

@test "Nested mount of a secret volume in a configmap volume for a pod" {
	# Creates a configmap, secret and pod that mounts the secret inside the configmap
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check config/secret value are correct
	[ "myconfig" == $(kubectl exec $pod_name -- "${exec_command1[@]}") ]
	[ "mysecret" == $(kubectl exec $pod_name -- "${exec_command2[@]}") ]
}

teardown() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"

	# Debugging information
	kubectl describe "pod/$pod_name"

	# Delete the configmap, secret, and pod used for testing
	kubectl delete -f "${yaml_file}"

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}
