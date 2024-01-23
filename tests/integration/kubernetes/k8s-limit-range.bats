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
	pod_yaml_file="${pod_config_dir}/pod-cpu-defaults.yaml"
}

@test "Limit range for storage" {
	auto_generate_policy "${pod_yaml_file}"

	# Create namespace
	kubectl create namespace "$namespace_name"

	# Create the LimitRange in the namespace
	kubectl create -f "${pod_config_dir}/limit-range.yaml" --namespace=${namespace_name}

	# Create the pod
	kubectl create -f "${pod_yaml_file}" --namespace=${namespace_name}

	# Get pod specification
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name" --namespace="$namespace_name"

	# Check limits
	# Find the 500 millicpus specified at the yaml
	kubectl describe pod "$pod_name" --namespace="$namespace_name" | grep "500m"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
	kubectl delete namespaces "$namespace_name"
}
