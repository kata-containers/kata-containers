#!/usr/bin/env bats
#
# Copyright (c) 2024 Microsoft.
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	get_pod_config_dir
	pod_name="nginxhttps"
	pod_yaml="${pod_config_dir}/pod-manifest-v1.yaml"
	auto_generate_policy "${pod_config_dir}" "${pod_yaml}"
}

@test "Deploy manifest v1 pod" {

	kubectl create -f "${pod_yaml}"

	# Wait for pod to start
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
}
