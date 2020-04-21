#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"
load "${BATS_TEST_DIRNAME}/../../lib/common.bash"
TEST_INITRD="${TEST_INITRD:-no}"
issue="https://github.com/kata-containers/runtime/issues/1127"
memory_issue="https://github.com/kata-containers/runtime/issues/1249"

setup() {
	skip "test not working see: ${issue}, ${memory_issue}"

	export KUBECONFIG="$HOME/.kube/config"
	pod_name="memory-test"
	get_pod_config_dir
}

@test "Exceeding memory constraints" {
	skip "test not working see: ${issue}, ${memory_issue}"

	memory_limit_size="50Mi"
	allocated_size="250M"
	# Create test .yaml
        sed \
            -e "s/\${memory_size}/${memory_limit_size}/" \
            -e "s/\${memory_allocated}/${allocated_size}/" \
            "${pod_config_dir}/pod-memory-limit.yaml" > "${pod_config_dir}/test_exceed_memory.yaml"

	# Create the pod exceeding memory constraints
	run kubectl create -f "${pod_config_dir}/test_exceed_memory.yaml"
	[ "$status" -ne 0 ]

	rm -f "${pod_config_dir}/test_exceed_memory.yaml"
}

@test "Running within memory constraints" {
	skip "test not working see: ${issue}, ${memory_issue}"

	memory_limit_size="200Mi"
	allocated_size="150M"
	# Create test .yaml
        sed \
            -e "s/\${memory_size}/${memory_limit_size}/" \
            -e "s/\${memory_allocated}/${allocated_size}/" \
            "${pod_config_dir}/pod-memory-limit.yaml" > "${pod_config_dir}/test_within_memory.yaml"

	# Create the pod within memory constraints
	kubectl create -f "${pod_config_dir}/test_within_memory.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready pod "$pod_name"

	rm -f "${pod_config_dir}/test_within_memory.yaml"
	kubectl delete pod "$pod_name"
}
