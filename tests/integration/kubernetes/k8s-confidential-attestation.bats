#!/usr/bin/env bats
# Copyright 2024 IBM Corporation
# Copyright 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"
load "${BATS_TEST_DIRNAME}/confidential_kbs.sh"

export KBS=${KBS:-false}
export test_key="aatest"
export SUPPORTED_TEE_HYPERVISORS=("qemu-sev" "qemu-snp" "qemu-tdx" "qemu-se")
export SUPPORTED_NON_TEE_HYPERVISORS=("qemu")

setup() {
	if ! [[ " ${KATA_HYPERVISOR} " =~ " ${SUPPORTED_TEE_HYPERVISORS[@]} " ]] && ! [[ " ${KATA_HYPERVISOR} " =~ " ${SUPPORTED_NON_TEE_HYPERVISORS} " ]]; then
		skip "Test not supported for ${KATA_HYPERVISOR}"
	fi

	if [ "${KBS}" = "false" ]; then
		skip "Test not supported for ${KATA_HYPERVISOR}"
	fi

	if [[ " ${SUPPORTED_NON_TEE_HYPERVISORS} " =~ " ${KATA_HYPERVISOR} " ]]; then
		info "Need to apply image annotations"
	else
		get_pod_config_dir
		setup_unencrypted_confidential_pod
	fi

	kbs_set_resource "default" "aa" "key" "$test_key"
	kbs_set_allow_all_resources
}

@test "Test we can get evidence from CDH resource" {
	local CC_KBS_ADDR
	CC_KBS_ADDR=$(kbs_k8s_svc_http_addr)
	# TODO - do we still need AA_KBC set and checked - do we have any other options?
	if [ "${AA_KBC}" = "cc_kbc" ]; then
		# TODO - Do we need to use different kernel params and attestation-agent doesn't get config from
		# kata-agent anymore based on Ding's PR comments
		add_kernel_params "agent.aa_kbc_params=cc_kbc::${CC_KBS_ADDR}"
	fi

	kubectl apply -f "${pod_config_dir}/pod-attestable.yaml"

	# Retrieve pod name, wait for it to come up, retrieve pod ip
	pod_name=$(kubectl get pod -o wide | grep "aa-test-cc" | awk '{print $1;}')

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout="$timeout" pod "${pod_name}"

	# Wait 5s for connecting with remote KBS
	sleep 5

	kubectl logs aa-test-cc
	kubectl logs aa-test-cc | grep -q "aatest"
}

teardown() {
	if ! [[ " ${KATA_HYPERVISOR} " =~ " ${SUPPORTED_TEE_HYPERVISORS[@]} " ]] && ! [[ " ${KATA_HYPERVISOR} " =~ " ${SUPPORTED_NON_TEE_HYPERVISORS} " ]]; then
		skip "Test not supported for ${KATA_HYPERVISOR}"
	fi

	kubectl describe "pod/${pod_name}" || true
	kubectl delete -f "${pod_config_dir}/pod-attestable.yaml" || true
}
