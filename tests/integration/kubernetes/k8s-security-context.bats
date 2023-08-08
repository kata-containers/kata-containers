#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"
load "${BATS_TEST_DIRNAME}/lib.sh"

setup() {
	get_pod_config_dir
}

@test "Security context" {
	pod_name="security-context-test"

	# Create pod
	create_pod_and_wait "${pod_config_dir}/pod-security-context.yaml" "$pod_name"

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
