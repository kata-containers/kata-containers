#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"
load "${BATS_TEST_DIRNAME}/../../lib/common.bash"

setup() {
	export KUBECONFIG="$HOME/.kube/config"
	pod_name="busybox"
	first_container_name="first-test-container"
	second_container_name="second-test-container"

	get_pod_config_dir
}

@test "Check PID namespaces" {
	# Create the pod
	kubectl create -f "${pod_config_dir}/busybox-pod.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready pod "$pod_name"

	# Check PID from first container
	first_pid_container=$(kubectl exec $pod_name -c $first_container_name ps | grep "/pause")

	# Check PID from second container
	second_pid_container=$(kubectl exec $pod_name -c $second_container_name ps | grep "/pause")

	[ "$first_pid_container" == "$second_pid_container" ]
}

teardown() {
	kubectl delete pod "$pod_name"
}
