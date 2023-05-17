#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	pod_name="busybox"
	first_container_name="first-test-container"
	second_container_name="second-test-container"

	get_pod_config_dir
}

@test "Check PID namespaces" {
	# Create the pod
	kubectl create -f "${pod_config_dir}/busybox-pod.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod $pod_name

	# Check PID from first container
	first_pid_container=$(kubectl exec $pod_name -c $first_container_name \
		-- ps | grep "/pause")
	# Verify that is not empty
	check_first_pid=$(echo $first_pid_container | wc -l)
	[ "$check_first_pid" == "1" ]

	# Check PID from second container
	second_pid_container=$(kubectl exec $pod_name -c $second_container_name \
		-- ps | grep "/pause")
	# Verify that is not empty
	check_second_pid=$(echo $second_pid_container | wc -l)
	[ "$check_second_pid" == "1" ]

	[ "$first_pid_container" == "$second_pid_container" ]
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
}
