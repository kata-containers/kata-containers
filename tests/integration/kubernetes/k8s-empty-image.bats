#!/usr/bin/env bats
#
# Copyright (c) 2025 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	setup_common || die "setup_common failed"
	pod_name="no-layer-image"
	get_pod_config_dir

	yaml_file="${pod_config_dir}/${pod_name}.yaml"
	auto_generate_policy "${pod_config_dir}" "${yaml_file}"
}

@test "Test image with no layers cannot run" {
	assert_pod_fail "${yaml_file}"
	kubectl get pods "${pod_name}" -o jsonpath='{.status.containerStatuses[0].lastState.terminated.message}' | grep "the file sleep was not found"
}

teardown() {
	# Debugging information
	kubectl describe "pod/${pod_name}"
	kubectl get "pod/${pod_name}" -o yaml

	kubectl delete pod "${pod_name}"

	node_end_time "${node}"
	echo "setup_common starts at ${node_start_time:-}, teardown_common ends at ${node_end_time:-}"
	teardown_common "${node}" "${node_start_time:-}"
}
