#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	get_pod_config_dir
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	cmd="env"
	exec_command=(sh -c "${cmd}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_command[@]}"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"

	configmap_yaml_file="${pod_config_dir}/configmap.yaml"
	pod_yaml_file="${pod_config_dir}/pod-configmap.yaml"

	auto_generate_policy "${policy_settings_dir}" "${pod_yaml_file}" "${configmap_yaml_file}"
}

@test "ConfigMap for a pod" {
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
	kubectl exec $pod_name -- "${exec_command[@]}" | grep "KUBE_CONFIG_1=value-1"
	kubectl exec $pod_name -- "${exec_command[@]}" | grep "KUBE_CONFIG_2=value-2"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
	kubectl delete configmap "$config_name"

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}
