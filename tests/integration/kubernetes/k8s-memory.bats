#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	pod_name="memory-test"
	get_pod_config_dir
}

setup_yaml() {
	sed \
		-e "s/\${memory_size}/${memory_limit_size}/" \
		-e "s/\${memory_allocated}/${allocated_size}/" \
		"${pod_config_dir}/pod-memory-limit.yaml"
}


@test "Exceeding memory constraints" {
	memory_limit_size="50Mi"
	allocated_size="250M"
	yaml_file="${pod_config_dir}/test_exceed_memory.yaml"

	# Create test .yaml
	setup_yaml > "${yaml_file}"

	auto_generate_policy "${yaml_file}"

	# Create the pod exceeding memory constraints
	run kubectl create -f "${yaml_file}"
	[ "$status" -ne 0 ]

	rm -f "${yaml_file}"
}

@test "Running within memory constraints" {
	memory_limit_size="600Mi"
	allocated_size="150M"
	yaml_file="${pod_config_dir}/test_within_memory.yaml"

	# Create test .yaml
	setup_yaml > "${yaml_file}"

	auto_generate_policy "${yaml_file}"

	# Create the pod within memory constraints
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	rm -f "${yaml_file}"
	kubectl delete pod "$pod_name"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name" || true
}
