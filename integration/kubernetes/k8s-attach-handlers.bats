#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"

setup() {
	export KUBECONFIG="$HOME/.kube/config"
	pod_name="handlers"

	if kubectl get runtimeclass | grep kata; then
		pod_config_dir="${BATS_TEST_DIRNAME}/runtimeclass_workloads"
	else
		pod_config_dir="${BATS_TEST_DIRNAME}/untrusted_workloads"
	fi
}

@test "Running with postStart and preStop handlers" {
	# Create the pod with postStart and preStop handlers
	kubectl create -f "${pod_config_dir}/lifecycle-events.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready pod "$pod_name"

	# Check postStart message
	display_message="cat /usr/share/message"
	check_postStart=$(kubectl exec $pod_name -- sh -c "$display_message" | grep "Hello from the postStart handler")
}

teardown(){
	kubectl delete pod "$pod_name"
}
