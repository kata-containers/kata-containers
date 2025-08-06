#!/usr/bin/env bats
#
# Copyright (c) 2024 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	pod_name="test-pod-hostname"
	get_pod_config_dir

	yaml_file="${pod_config_dir}/pod-hostname.yaml"
	add_allow_all_policy_to_yaml "${yaml_file}"

	expected_name=$pod_name
}

@test "Validate Pod hostname" {
	# Create pod
	kubectl apply -f "${yaml_file}"

	kubectl wait --for jsonpath=status.phase=Succeeded --timeout=$timeout pod "$pod_name"

	# Validate the pod hostname
	result=$(kubectl logs $pod_name)
	[ "$pod_name" == "$result" ]
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
}
