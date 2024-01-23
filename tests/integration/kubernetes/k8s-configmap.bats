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
	configmap_yaml_file="${pod_config_dir}/configmap.yaml"
	pod_yaml_file="${pod_config_dir}/pod-configmap.yaml"
}

@test "ConfigMap for a pod" {
	config_name="test-configmap"
	pod_name="config-env-test-pod"

	# TODO: disabled due to #8850
	# auto_generate_policy "${pod_yaml_file}" "${configmap_yaml_file}"

	# Create ConfigMap
	kubectl create -f "${configmap_yaml_file}"

	# View the values of the keys
	kubectl get configmaps $config_name -o yaml | grep -q "data-"

	# Create a pod that consumes the ConfigMap
	kubectl create -f "${pod_yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check env
	cmd="env"
	kubectl exec $pod_name -- sh -c $cmd | grep "KUBE_CONFIG_1=value-1"
	kubectl exec $pod_name -- sh -c $cmd | grep "KUBE_CONFIG_2=value-2"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
	kubectl delete configmap "$config_name"
}
