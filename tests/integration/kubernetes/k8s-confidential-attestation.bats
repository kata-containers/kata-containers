#!/usr/bin/env bats
# Copyright 2024 IBM Corporation
# Copyright 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"
load "${BATS_TEST_DIRNAME}/confidential_kbs.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

export KBS="${KBS:-false}"
export test_key="aatest"
export SUPPORTED_TEE_HYPERVISORS=("qemu-sev" "qemu-snp" "qemu-tdx" "qemu-se")
export SUPPORTED_NON_TEE_HYPERVISORS=("qemu")
export KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"
export AA_KBC="${AA_KBC:-cc_kbc}"

setup() {
	if ! [[ " ${SUPPORTED_TEE_HYPERVISORS[@]} " =~ " ${KATA_HYPERVISOR} " ]] && ! [[ " ${SUPPORTED_NON_TEE_HYPERVISORS} " =~ " ${KATA_HYPERVISOR} " ]]; then
		skip "Test not supported for ${KATA_HYPERVISOR}"
	fi

	if [ "${KBS}" = "false" ]; then
		skip "Test not supported for ${KATA_HYPERVISOR}"
	fi

	get_pod_config_dir
	
#	setup_unencrypted_confidential_pod

	export K8S_TEST_YAML="${pod_config_dir}/pod-attestable.yaml"

	if [[ " ${SUPPORTED_NON_TEE_HYPERVISORS} " =~ " ${KATA_HYPERVISOR} " ]]; then
		info "Need to apply image annotations"
		export add_non_tee_annotation="io.katacontainers.config.hypervisor.image"
		export non_tee_image_path="/opt/kata/share/kata-containers/kata-containers-confidential.img"
		yq write -i "${K8S_TEST_YAML}" "metadata.annotations[${add_non_tee_annotation}]" "${non_tee_image_path}"
	fi

	kbs_set_resource "default" "aa" "key" "$test_key"
	kbs_set_allow_all_resources
}

@test "Get CDH resource" {
	local CC_KBS_ADDR
	export CC_KBS_ADDR=$(kbs_k8s_svc_http_addr)
	# TODO - do we still need AA_KBC set and checked - do we have any other options?
	if [ "${AA_KBC}" = "cc_kbc" ]; then
		export aa_kbc_annotation="io.katacontainers.config.hypervisor.kernel_params"
		export aa_kbc_value="agent.aa_kbc_params=cc_kbc::${CC_KBS_ADDR}"
		yq write -i "${K8S_TEST_YAML}" "metadata.annotations[${aa_kbc_annotation}]" "${aa_kbc_value}"
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

teardown() {
	if ! [[ " ${SUPPORTED_TEE_HYPERVISORS[@]} " =~ " ${KATA_HYPERVISOR} " ]] && ! [[ " ${SUPPORTED_NON_TEE_HYPERVISORS} " =~ " ${KATA_HYPERVISOR} " ]]; then
		skip "Test not supported for ${KATA_HYPERVISOR}"
	fi

	[ -n "${pod_name:-}" ] && kubectl describe "pod/${pod_name}" || true
	[ -n "${pod_config_dir:-}" ] && kubectl delete -f "${K8S_TEST_YAML}" || true
}
