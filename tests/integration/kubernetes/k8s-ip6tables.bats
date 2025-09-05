#!/usr/bin/env bats
#
# Copyright (c) 2025 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	[ "$(uname -m)" == "ppc64le" ] && skip "ip6tables tests for ppc64le"

	setup_common
	pod_name="pod-istio"
	get_pod_config_dir

	yaml_file="${pod_config_dir}/pod-istio.yaml"
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

@test "Pod that performs ip6tables setup" {
	kubectl apply -f "${yaml_file}"

	# Check pod completion
	kubectl wait --for=jsonpath="status.containerStatuses[0].state.terminated.reason"=Completed --timeout=$timeout pod "$pod_name"

	# Verify that the job is completed
	cmd="kubectl get pods -o jsonpath='{.items[*].status.phase}' | grep Succeeded"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Verify the output of the pod
	success_criterion="COMMIT"
	kubectl logs "$pod_name" | grep "$success_criterion"
}

teardown() {
	[ "$(uname -m)" == "ppc64le" ] && skip "ip6tables tests for ppc64le"
	
	# Debugging information
	kubectl logs "$pod_name"

	teardown_common "${node}" "${node_start_time:-}" "${policy_settings_dir}"
}
