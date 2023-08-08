#!/usr/bin/env bats
#
# Copyright (c) 2022 AntGroup Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"
load "${BATS_TEST_DIRNAME}/lib.sh"

setup() {
	pod_name="busybox"
	first_container_name="first-test-container"

	get_pod_config_dir
}

@test "Check PID namespaces" {
	# Create the pod
	create_pod_and_wait "${pod_config_dir}/initcontainer-shareprocesspid.yaml" "$pod_name"

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
