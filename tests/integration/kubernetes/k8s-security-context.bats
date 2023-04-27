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
}

@test "Security context" {
	pod_name="security-context-test"

	# Create pod
	kubectl create -f "${pod_config_dir}/pod-security-context.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check user
	cmd="ps --user 1000 -f"
	process="tail -f /dev/null"
	kubectl exec $pod_name -- sh -c $cmd | grep "$process"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
}
