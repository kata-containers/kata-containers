#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"

setup() {
	export KUBECONFIG=/etc/kubernetes/admin.conf
	pod_name="liveness-exec"
	pod_config_dir="${BATS_TEST_DIRNAME}/untrusted_workloads"
}

@test "Liveness probe" {
	sleep_liveness=10

	# Create pod
	sudo -E kubectl create -f "${pod_config_dir}/pod-liveness.yaml"

	# Check pod creation
	sudo -E kubectl wait --for=condition=Ready pod "$pod_name"

	# Check liveness probe returns a success code
	sudo -E kubectl describe pod "$pod_name" | grep -E "Liveness|#success=1"

	# Sleep necessary to check liveness probe returns a failure code
	sleep "$sleep_liveness"
	sudo -E kubectl describe pod "$pod_name" | grep "Liveness probe failed"
}

teardown() {
	sudo -E kubectl delete pod "$pod_name"
}
