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
	yaml_file="${pod_config_dir}/pod-optional-empty-configmap.yaml"
}

@test "Optional and Empty ConfigMap Volume for a pod" {
	config_name="empty-config"
	pod_name="optional-empty-config-test-pod"

	# TODO: disabled due to #8893
	# auto_generate_policy "${yaml_file}"

	# Create Empty ConfigMap
	kubectl create configmap "$config_name"

	# Create a pod that consumes the "empty-config" and "optional-missing-config" ConfigMaps as volumes
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check configmap folders exist
	kubectl exec $pod_name -- sh -c ls /empty-config
	kubectl exec $pod_name -- sh -c ls /optional-missing-config
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
	kubectl delete configmap "$config_name"
}
