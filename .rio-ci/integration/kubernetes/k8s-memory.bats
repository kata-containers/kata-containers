#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/config}"
	pod_name="memory-test"
	get_pod_config_dir
}

@test "Exceeding memory constraints" {
	memory_limit_size="50Mi"
	allocated_size="250M"
	# Create the pod exceeding memory constraints
	pcl -e MEMORY_ALLOCATED="${allocated_size}" -e MEMORY_SIZE="${memory_limit_size}" \
	"${pod_config_dir}/pod-memory-limit.pcl" > "${pod_config_dir}/test_exceed_memory.yaml"
	run kubectl create -f "${pod_config_dir}/test_exceed_memory.yaml"

	[ "$status" -ne 0 ]
	rm -f "${pod_config_dir}/test_exceed_memory.yaml"
}

@test "Running within memory constraints" {
	memory_limit_size="600Mi"
	allocated_size="150M"

	# Create the pod within memory constraints
	pcl -e MEMORY_ALLOCATED="${allocated_size}" -e MEMORY_SIZE="${memory_limit_size}" \
	"${pod_config_dir}/pod-memory-limit.pcl" | kubectl create -f -

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	kubectl delete pod "$pod_name"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name" || true
}
