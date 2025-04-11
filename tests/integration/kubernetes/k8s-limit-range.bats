#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	get_pod_config_dir
	namespace_name="default-cpu-example"
	pod_name="default-cpu-test"
	pod_yaml="${pod_config_dir}/pod-cpu-defaults.yaml"

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	auto_generate_policy "${policy_settings_dir}" "${pod_yaml}"
}

@test "Limit range for storage" {
	# Create namespace
	kubectl create namespace "$namespace_name"

	# Create the LimitRange in the namespace
	kubectl create -f "${pod_config_dir}/limit-range.yaml" --namespace=${namespace_name}

	# Create the pod
	kubectl create -f "${pod_yaml}" --namespace=${namespace_name}

	# Get pod specification
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name" --namespace="$namespace_name"

	# Check limits
	# Find the 500 millicpus specified at the yaml
	kubectl describe pod "$pod_name" --namespace="$namespace_name" | grep "500m"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name" -n "$namespace_name"

	kubectl delete pod "$pod_name" -n "$namespace_name"
	kubectl delete namespaces "$namespace_name"

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}
