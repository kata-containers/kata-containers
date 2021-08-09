#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/config}"
	get_pod_config_dir
}

@test "Launch a 50mCPU pod" {
	config_name="test-tiny"
	pod_name="pod-tiny"

	# Create a tiny pod
	pcl "${pod_config_dir}/pod-tiny.pcl" | kubectl create -f -

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=270s pod "$pod_name"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"
	kubectl delete pod "$pod_name"
}
