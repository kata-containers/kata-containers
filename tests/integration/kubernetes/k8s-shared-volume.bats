#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"
load "${BATS_TEST_DIRNAME}/lib.sh"

setup() {
	get_pod_config_dir
}

@test "Containers with shared volume" {
	pod_name="test-shared-volume"
	first_container_name="busybox-first-container"
	second_container_name="busybox-second-container"

	# Create pod
	create_pod_and_wait "${pod_config_dir}/pod-shared-volume.yaml" "$pod_name"

	# Communicate containers
	cmd="cat /tmp/pod-data"
	msg="Hello from the $second_container_name"
	kubectl exec "$pod_name" -c "$first_container_name" -- sh -c "$cmd" | grep "$msg"
}

@test "initContainer with shared volume" {
	pod_name="initcontainer-shared-volume"
	last_container="last"

	# Create pod
	create_pod_and_wait "${pod_config_dir}/initContainer-shared-volume.yaml" "$pod_name"

	cmd='test $(cat /volume/initContainer) -lt $(cat /volume/container)'
	kubectl exec "$pod_name" -c "$last_container" -- sh -c "$cmd"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
}
