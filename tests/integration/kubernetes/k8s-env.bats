#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	pod_name="test-env"
	get_pod_config_dir
	yaml_file="${pod_config_dir}/pod-env.yaml"
}

@test "Environment variables" {
	# TODO: disabled due to #8850
	# auto_generate_policy "${yaml_file}"

	# Create pod
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Print environment variables
	cmd="printenv"
	kubectl exec $pod_name -- sh -c $cmd | grep "MY_POD_NAME=$pod_name"
	kubectl exec $pod_name -- sh -c $cmd | \
		grep "HOST_IP=\([0-9]\+\(\.\|$\)\)\{4\}"
	# Requested 32Mi of memory
	kubectl exec $pod_name -- sh -c $cmd | \
		grep "MEMORY_REQUESTS=$((1024 * 1024 * 32))"
	# Memory limits allocated by the node
	kubectl exec $pod_name -- sh -c $cmd | grep "MEMORY_LIMITS=[1-9]\+"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
}
