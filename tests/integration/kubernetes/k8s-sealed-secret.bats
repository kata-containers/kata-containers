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
	[ "${KATA_HYPERVISOR}" = "qemu-coco-dev" ] || skip "Test not ready yet for ${KATA_HYPERVISOR}"

	if [ "${KBS}" = "false" ]; then
		skip "Test skipped as KBS not setup"
	fi

	setup_common || die "setup_common failed"
	get_pod_config_dir

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

	# Setup k8s secret
	kubectl delete secret sealed-secret --ignore-not-found
	kubectl delete secret not-sealed-secret --ignore-not-found

	# Sealed secret format is defined at: https://github.com/confidential-containers/guest-components/blob/main/confidential-data-hub/docs/SEALED_SECRET.md#vault
	# sealed.BASE64URL(UTF8(JWS Protected Header)) || '.
	# || BASE64URL(JWS Payload) || '.'
	# || BASE64URL(JWS Signature)
	# test payload:
	# {
	# "version": "0.1.0",
	# "type": "vault",
	# "name": "kbs:///default/sealed-secret/test",
	# "provider": "kbs",
	# "provider_settings": {},
	# "annotations": {}
	# }
	kubectl create secret generic sealed-secret --from-literal='secret=sealed.fakejwsheader.eyJ2ZXJzaW9uIjoiMC4xLjAiLCJ0eXBlIjoidmF1bHQiLCJuYW1lIjoia2JzOi8vL2RlZmF1bHQvc2VhbGVkLXNlY3JldC90ZXN0IiwicHJvdmlkZXIiOiJrYnMiLCJwcm92aWRlcl9zZXR0aW5ncyI6e30sImFubm90YXRpb25zIjp7fX0.fakesignature'

	kubectl create secret generic not-sealed-secret --from-literal='secret=not_sealed_secret'

	if ! is_confidential_hardware; then
		kbs_set_allow_all_resources
	fi
}

@test "Cannot Unseal Env Secrets with CDH without key" {
	k8s_create_pod "${K8S_TEST_ENV_YAML}"

	kubectl logs secret-test-pod-cc
	kubectl logs secret-test-pod-cc | grep -q "UNPROTECTED_SECRET = not_sealed_secret"
	cmd="kubectl logs secret-test-pod-cc | grep -q \"PROTECTED_SECRET = unsealed_secret\""
	run $cmd
	[ "$status" -eq 1 ]
}


@test "Unseal Env Secrets with CDH" {
	kbs_set_resource "default" "sealed-secret" "test" "unsealed_secret"
	k8s_create_pod "${K8S_TEST_ENV_YAML}"

	kubectl logs secret-test-pod-cc
	kubectl logs secret-test-pod-cc | grep -q "UNPROTECTED_SECRET = not_sealed_secret"
	kubectl logs secret-test-pod-cc | grep -q "PROTECTED_SECRET = unsealed_secret"
}

@test "Unseal File Secrets with CDH" {
	kbs_set_resource "default" "sealed-secret" "test" "unsealed_secret"
	k8s_create_pod "${K8S_TEST_FILE_YAML}"

	kubectl logs secret-test-pod-cc
	kubectl logs secret-test-pod-cc | grep -q "UNPROTECTED_SECRET = not_sealed_secret"
	kubectl logs secret-test-pod-cc | grep -q "PROTECTED_SECRET = unsealed_secret"
}

teardown() {
	[ "${KATA_HYPERVISOR}" = "qemu-coco-dev" ] || skip "Test not ready yet for ${KATA_HYPERVISOR}"

	if [ "${KBS}" = "false" ]; then
		skip "Test skipped as KBS not setup"
	fi

	teardown_common "${node}" "${node_start_time:-}"
	kubectl delete secret sealed-secret --ignore-not-found
	kubectl delete secret not-sealed-secret --ignore-not-found
}
