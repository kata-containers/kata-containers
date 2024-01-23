#!/usr/bin/env bats
#
# Copyright (c) 2020 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	[ "${KATA_HOST_OS}" == "cbl-mariner" ] && skip "test not working see: see #8821"

	pod_name="pod-oom"
	get_pod_config_dir
	yaml_file="${pod_config_dir}/$pod_name.yaml"
}

@test "Test OOM events for pods" {
	auto_generate_policy "${yaml_file}"

	# Create pod
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check if OOMKilled
	cmd="kubectl get pods "$pod_name" -o jsonpath='{.status.containerStatuses[0].state.terminated.reason}' | grep OOMKilled"

	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	rm -f "${pod_config_dir}/test_pod_oom.yaml"
}

teardown() {
	[ "${KATA_HOST_OS}" == "cbl-mariner" ] && skip "test not working see: see #8821"

	# Debugging information
	kubectl describe "pod/$pod_name"
	kubectl get "pod/$pod_name" -o yaml

	kubectl delete pod "$pod_name"
}
