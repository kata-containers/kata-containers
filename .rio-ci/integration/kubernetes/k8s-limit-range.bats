#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/config}"
	get_pod_config_dir
	namespace_name="default-cpu-example"
	pod_name="default-cpu-test"
}

@test "Limit range for storage" {
	# Create namespace
	kubectl create namespace "$namespace_name"

	# Create the LimitRange in the namespace
	pcl "${pod_config_dir}/limit-range.pcl" | kubectl --namespace=${namespace_name} create -f -

	# Create the pod
	pcl "${pod_config_dir}/pod-cpu-defaults.pcl" | kubectl --namespace=${namespace_name} create -f -

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
