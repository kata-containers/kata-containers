#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"

setup() {
	export KUBECONFIG=/etc/kubernetes/admin.conf
	pod_name="handlers"
	pod_config_dir="${BATS_TEST_DIRNAME}/untrusted_workloads"
}

@test "Running with postStart and preStop handlers" {
	# Create the pod with postStart and preStop handlers
	sudo -E kubectl create -f "${pod_config_dir}/lifecycle-events.yaml"

	# Check pod creation
	sudo -E kubectl wait --for=condition=Ready pod "$pod_name"

	# Check postStart message
	display_message="cat /usr/share/message"
	check_postStart=$(sudo -E kubectl exec $pod_name -- sh -c "$display_message" | grep "Hello from the postStart handler")
}

teardown(){
	sudo -E kubectl delete pod "$pod_name"
}
