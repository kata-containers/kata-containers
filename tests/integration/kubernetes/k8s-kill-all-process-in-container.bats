#!/usr/bin/env bats
#
# Copyright (c) 2022 AntGroup Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	pod_name="busybox"
	first_container_name="first-test-container"

	get_pod_config_dir
}

@test "Kill all processes in container" {
	# Create the pod
	kubectl create -f "${pod_config_dir}/initcontainer-shareprocesspid.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod $pod_name

	# Check PID from first container
	first_pid_container=$(kubectl exec $pod_name -c $first_container_name \
		-- ps | grep "tail" || true)
	# Verify that the tail process didn't exist
	[ -z $first_pid_container ] || die "found processes pid: $first_pid_container" 
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
}
