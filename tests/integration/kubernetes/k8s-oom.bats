#!/usr/bin/env bats
#
# Copyright (c) 2020 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	pod_name="pod-oom"
	get_pod_config_dir

	yaml_file="${pod_config_dir}/$pod_name.yaml"
	auto_generate_policy "${pod_config_dir}" "${yaml_file}"
}

@test "Test OOM events for pods" {
	# Create pod
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=100s pod "$pod_name"

	# Check if OOMKilled
    container_name=$(kubectl get pod "$pod_name" -o jsonpath='{.status.containerStatuses[0].name}')
    if [[ $container_name == "oom-test" ]]; then
        cmd="kubectl get pods "$pod_name" -o jsonpath='{.status.containerStatuses[0].state.terminated.reason}' | grep OOMKilled"
    else
        cmd="kubectl get pods "$pod_name" -o jsonpath='{.status.containerStatuses[1].state.terminated.reason}' | grep OOMKilled"
    fi

	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	rm -f "${pod_config_dir}/test_pod_oom.yaml"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"
	kubectl get "pod/$pod_name" -o yaml

	kubectl delete pod "$pod_name"
}
