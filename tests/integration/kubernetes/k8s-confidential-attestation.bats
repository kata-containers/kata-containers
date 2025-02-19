#!/usr/bin/env bats
# Copyright 2024 IBM Corporation
# Copyright 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"

export KBS="${KBS:-false}"
export test_key="aatest"
export KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"
export AA_KBC="${AA_KBC:-cc_kbc}"

setup() {
	is_confidential_runtime_class || skip "Test not supported for ${KATA_HYPERVISOR}."

	if [ "${KBS}" = "false" ]; then
		skip "Test skipped as KBS not setup"
	fi

	setup_common
	get_pod_config_dir

#	setup_unencrypted_confidential_pod

	export K8S_TEST_YAML="${pod_config_dir}/pod-attestable.yaml"
	# Schedule on a known node so that later it can print the system's logs for
	# debugging.
	set_node "$K8S_TEST_YAML" "$node"

	kbs_set_resource "default" "aa" "key" "$test_key"
	local CC_KBS_ADDR
	export CC_KBS_ADDR=$(kbs_k8s_svc_http_addr)
	kernel_params_annotation="io.katacontainers.config.hypervisor.kernel_params"
	kernel_params_value="agent.guest_components_rest_api=resource"
	# Based on current config we still need to pass the agent.aa_kbc_params, but this might change
	# as the CDH/Attestaiton-agent config gets updated
	if [ "${AA_KBC}" = "cc_kbc" ]; then
		kernel_params_value+=" agent.aa_kbc_params=cc_kbc::${CC_KBS_ADDR}"
	fi
	set_metadata_annotation "${K8S_TEST_YAML}" \
		"${kernel_params_annotation}" \
		"${kernel_params_value}"
}

@test "Get CDH resource" {
	if ! is_confidential_hardware; then
		kbs_set_allow_all_resources
	fi

	kubectl apply -f "${K8S_TEST_YAML}"

	# Retrieve pod name, wait for it to come up, retrieve pod ip
	export pod_name=$(kubectl get pod -o wide | grep "aa-test-cc" | awk '{print $1;}')

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout="$timeout" pod "${pod_name}"

	# Wait 5s for connecting with remote KBS
	sleep 5

	kubectl logs aa-test-cc
	kubectl logs aa-test-cc | grep -q "aatest"
}

@test "Cannot get CDH resource when deny-all policy is set" {
	kbs_set_deny_all_resources
	kubectl apply -f "${K8S_TEST_YAML}"

	# Retrieve pod name, wait for it to come up, retrieve pod ip
	export pod_name=$(kubectl get pod -o wide | grep "aa-test-cc" | awk '{print $1;}')

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout="$timeout" pod "${pod_name}"

	sleep 5

	kubectl logs aa-test-cc
	cmd="kubectl logs aa-test-cc | grep -q aatest"
	run $cmd
	[ "$status" -eq 1 ]
}

teardown() {
	is_confidential_runtime_class || skip "Test not supported for ${KATA_HYPERVISOR}."

	if [ "${KBS}" = "false" ]; then
		skip "Test skipped as KBS not setup"
	fi

	teardown_common "${node}" "${node_start_time:-}"
}
