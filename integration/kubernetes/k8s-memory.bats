#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"

setup() {
	export KUBECONFIG=/etc/kubernetes/admin.conf
	pod_name="memory-test"
}

@test "Exceeding memory constraints" {
	memory_limit_size="50Mi"
	allocated_size="250M"
	# Create test .yaml
        sed \
            -e "s/\${memory_size}/${memory_limit_size}/" \
            -e "s/\${memory_allocated}/${allocated_size}/" \
            pod-memory-limit.yaml > test_exceed_memory.yaml

	# Create the pod exceeding memory constraints
	run sudo -E kubectl create -f test_exceed_memory.yaml
	[ "$status" -ne 0 ]

	rm -f test_exceed_memory.yaml
}

@test "Running within memory constraints" {
	memory_limit_size="200Mi"
	allocated_size="150M"
	wait_time=120
	sleep_time=5
	# Create test .yaml
        sed \
            -e "s/\${memory_size}/${memory_limit_size}/" \
            -e "s/\${memory_allocated}/${allocated_size}/" \
            pod-memory-limit.yaml > test_within_memory.yaml

	# Create the pod within memory constraints
	sudo -E kubectl create -f test_within_memory.yaml

	# Check pod creation
	pod_status_cmd="sudo -E kubectl get pods -a | grep $pod_name | grep Running"
	waitForProcess "$wait_time" "$sleep_time" "$pod_status_cmd"

	rm -f test_within_memory.yaml
	sudo -E kubectl delete pod "$pod_name"
}
