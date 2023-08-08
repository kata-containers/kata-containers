#!/usr/bin/env bats
#
# Copyright (c) 2020 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"
load "${BATS_TEST_DIRNAME}/lib.sh"

setup() {
	pod_name="pod-oom"
	get_pod_config_dir
}

@test "Test OOM events for pods" {
	# Create pod
	create_pod_and_wait "${pod_config_dir}/$pod_name.yaml" "$pod_name"

	# Check if OOMKilled
	cmd="kubectl get pods "$pod_name" -o jsonpath='{.status.containerStatuses[0].state.terminated.reason}' | grep OOMKilled"

	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	rm -f "${pod_config_dir}/test_pod_oom.yaml"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"
	kubectl get "pod/$pod_name" -o yaml

	kubectl delete pod "$pod_name"
}
