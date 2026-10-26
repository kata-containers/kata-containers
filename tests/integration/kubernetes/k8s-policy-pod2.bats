#!/usr/bin/env bats
#
# Copyright (c) 2024 Microsoft.
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	auto_generate_policy_enabled || skip "Auto-generated policy tests are disabled."
	setup_common || die "setup_common failed"

	pod_name="policy-pod2"
	yaml_file="${pod_config_dir}/k8s-policy-pod2.yaml"
	auto_generate_policy "${pod_config_dir}" "${yaml_file}"
}

@test "Successful pod start" {
	kubectl create -f "${yaml_file}"
	cmd="kubectl wait --for=condition=Ready --timeout=0s pod ${pod_name}"
	abort_cmd="kubectl describe pod ${pod_name} | grep \"CreateContainerRequest is blocked by policy\""
	info "Waiting ${wait_time}s with sleep ${sleep_time}s for: ${cmd}. Abort if: ${abort_cmd}."
	waitForCmdWithAbortCmd "${wait_time}" "${sleep_time}" "${cmd}" "${abort_cmd}"
}

teardown() {
	auto_generate_policy_enabled || skip "Auto-generated policy tests are disabled."

	# Debugging information.
	kubectl describe pod "${pod_name}"

	# Clean-up
	kubectl delete pod "${pod_name}"
	teardown_common "${node}" "${node_start_time:-}"
}
