#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	setup_common || die "setup_common failed"

	pod_name="default-cpu-test"
	pod_yaml="${pod_config_dir}/pod-cpu-defaults.yaml"

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	auto_generate_policy "${policy_settings_dir}" "${pod_yaml}"
}

@test "Limit range for storage" {
	# Use the suite's namespace so concurrent runs remain isolated.
	kubectl create -f "${pod_config_dir}/limit-range.yaml"

	# Create the pod
	kubectl create -f "${pod_yaml}"

	# Get pod specification
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check limits
	# Find the 500 millicpus specified at the yaml
	kubectl describe pod "$pod_name" | grep "500m"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name" || true

	kubectl delete pod "$pod_name" --ignore-not-found=true
	kubectl delete -f "${pod_config_dir}/limit-range.yaml" --ignore-not-found=true

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
	teardown_common "${node}" "${node_start_time:-}"
}
