#!/usr/bin/env bats
# Copyright 2024 IBM Corporation
# Copyright 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Test for Sealed Secret feature of CoCo
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"

export KBS="${KBS:-false}"
export KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"
export AA_KBC="${AA_KBC:-cc_kbc}"

setup() {
	if ! is_confidential_runtime_class; then
		skip "Test not supported for ${KATA_HYPERVISOR}."
	fi

	if [ "${KBS}" = "false" ]; then
		skip "Test skipped as KBS not setup"
	fi

	setup_common || die "setup_common failed"

	export K8S_TEST_ENV_YAML="${pod_config_dir}/pod-sealed-secret.yaml"
	export K8S_TEST_FILE_YAML="${pod_config_dir}/pod-sealed-secret-as-file.yaml"
	# Schedule on a known node so that later it can print the system's logs for
	# debugging.
	set_node "$K8S_TEST_ENV_YAML" "$node"
	set_node "$K8S_TEST_FILE_YAML" "$node"

	local CC_KBS_ADDR
	export CC_KBS_ADDR=$(kbs_k8s_svc_http_addr)
	kernel_params_annotation="io.katacontainers.config.hypervisor.kernel_params"
	kernel_params_value="agent.guest_components_procs=confidential-data-hub"

	# For now we set aa_kbc_params via kernel cmdline
	if [ "${AA_KBC}" = "cc_kbc" ]; then
		kernel_params_value+=" agent.aa_kbc_params=cc_kbc::${CC_KBS_ADDR}"
	fi
	set_metadata_annotation "${K8S_TEST_ENV_YAML}" \
		"${kernel_params_annotation}" \
		"${kernel_params_value}"
	set_metadata_annotation "${K8S_TEST_FILE_YAML}" \
		"${kernel_params_annotation}" \
		"${kernel_params_value}"

	# provision signing public key to KBS so that CDH can verify pre-created, signed secret.
	setup_sealed_secret_signing_public_key

	# Setup k8s secret
	kubectl delete secret sealed-secret --ignore-not-found
	kubectl delete secret not-sealed-secret --ignore-not-found
	kubectl create secret generic sealed-secret --from-literal="secret=${SEALED_SECRET_PRECREATED_TEST}"
	kubectl create secret generic not-sealed-secret --from-literal='secret=not_sealed_secret'

	if ! is_confidential_hardware; then
		kbs_set_allow_all_resources
	else
		kbs_set_default_policy
	fi
}

@test "Cannot Unseal Env Secrets with CDH without key" {
	k8s_create_pod "${K8S_TEST_ENV_YAML}"

	logs=$(kubectl logs secret-test-pod-cc)
	echo "$logs"
	grep -q "UNPROTECTED_SECRET = not_sealed_secret" <<< "$logs"
	run grep -q "PROTECTED_SECRET = unsealed_secret" <<< "$logs"
	[ "$status" -eq 1 ]
}


@test "Unseal Env Secrets with CDH" {
	kbs_set_resource "default" "sealed-secret" "test" "unsealed_secret"
	k8s_create_pod "${K8S_TEST_ENV_YAML}"

	logs=$(kubectl logs secret-test-pod-cc)
	echo "$logs"
	grep -q "UNPROTECTED_SECRET = not_sealed_secret" <<< "$logs"
	grep -q "PROTECTED_SECRET = unsealed_secret" <<< "$logs"
}

@test "Unseal File Secrets with CDH" {
	kbs_set_resource "default" "sealed-secret" "test" "unsealed_secret"
	k8s_create_pod "${K8S_TEST_FILE_YAML}"

	logs=$(kubectl logs secret-test-pod-cc)
	echo "$logs"
	grep -q "UNPROTECTED_SECRET = not_sealed_secret" <<< "$logs"
	grep -q "PROTECTED_SECRET = unsealed_secret" <<< "$logs"
}

teardown() {
	if ! is_confidential_runtime_class; then
		skip "Test not supported for ${KATA_HYPERVISOR}."
	fi

	if [ "${KBS}" = "false" ]; then
		skip "Test skipped as KBS not setup"
	fi

	confidential_teardown_common "${node}" "${node_start_time:-}"
	kubectl delete secret sealed-secret --ignore-not-found
	kubectl delete secret not-sealed-secret --ignore-not-found
}
