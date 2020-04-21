#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"
load "${BATS_TEST_DIRNAME}/../../lib/common.bash"

setup() {
	export KUBECONFIG="$HOME/.kube/config"
	get_pod_config_dir
	namespace_name="default-cpu-example"
	pod_name="default-cpu-test"
}

@test "Limit range for storage" {
	# Create namespace
	kubectl create namespace "$namespace_name"

	# Create the LimitRange in the namespace
	kubectl create -f "${pod_config_dir}/limit-range.yaml" --namespace=${namespace_name}

	# Create the pod
	kubectl create -f "${pod_config_dir}/pod-cpu-defaults.yaml" --namespace=${namespace_name}

	# Get pod specification
	kubectl wait --for=condition=Ready pod "$pod_name" --namespace="$namespace_name"

	# Check limits
	# Find the 500 millicpus specified at the yaml
	kubectl describe pod "$pod_name" --namespace="$namespace_name" | grep "500m"
}

teardown() {
	kubectl delete pod "$pod_name"
	kubectl delete namespaces "$namespace_name"
}
