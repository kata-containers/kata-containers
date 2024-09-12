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

	setup_common
	get_pod_config_dir

	export K8S_TEST_YAML="${pod_config_dir}/pod-sealed-secret.yaml"
	# Schedule on a known node so that later it can print the system's logs for
	# debugging.
	set_node "$K8S_TEST_YAML" "$node"

	local CC_KBS_ADDR
	export CC_KBS_ADDR=$(kbs_k8s_svc_http_addr)
	kernel_params_annotation="io.katacontainers.config.hypervisor.kernel_params"
	kernel_params_value="agent.guest_components_procs=confidential-data-hub"

	# For now we set aa_kbc_params via kernel cmdline
	if [ "${AA_KBC}" = "cc_kbc" ]; then
		kernel_params_value+=" agent.aa_kbc_params=cc_kbc::${CC_KBS_ADDR}"
	fi
	set_metadata_annotation "${K8S_TEST_YAML}" \
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
	kubectl create secret generic sealed-secret --from-literal='secret=sealed.fakejwsheader.ewogICAgInZlcnNpb24iOiAiMC4xLjAiLAogICAgInR5cGUiOiAidmF1bHQiLAogICAgIm5hbWUiOiAia2JzOi8vL2RlZmF1bHQvc2VhbGVkLXNlY3JldC90ZXN0IiwKICAgICJwcm92aWRlciI6ICJrYnMiLAogICAgInByb3ZpZGVyX3NldHRpbmdzIjoge30sCiAgICAiYW5ub3RhdGlvbnMiOiB7fQp9Cg==.fakesignature'

	kubectl create secret generic not-sealed-secret --from-literal='secret=not_sealed_secret'

	if ! is_confidential_hardware; then
		kbs_set_allow_all_resources
	fi
}

@test "Cannot Unseal Env Secrets with CDH without key" {
	[ "${KATA_HYPERVISOR}" = "qemu-coco-dev" ] || skip "Test not ready yet for ${KATA_HYPERVISOR}"

	if [ "${KBS}" = "false" ]; then
		skip "Test skipped as KBS not setup"
	fi

	k8s_create_pod "${K8S_TEST_YAML}"

	kubectl logs secret-test-pod-cc
	kubectl logs secret-test-pod-cc | grep -q "UNPROTECTED_SECRET = not_sealed_secret"
	cmd="kubectl logs secret-test-pod-cc | grep -q \"PROTECTED_SECRET = unsealed_secret\""
	run $cmd
	[ "$status" -eq 1 ]
}


@test "Unseal Env Secrets with CDH" {
	[ "${KATA_HYPERVISOR}" = "qemu-coco-dev" ] || skip "Test not ready yet for ${KATA_HYPERVISOR}"

	if [ "${KBS}" = "false" ]; then
		skip "Test skipped as KBS not setup"
	fi

	kbs_set_resource "default" "sealed-secret" "test" "unsealed_secret"
	k8s_create_pod "${K8S_TEST_YAML}"

	kubectl logs secret-test-pod-cc
	kubectl logs secret-test-pod-cc | grep -q "UNPROTECTED_SECRET = not_sealed_secret"
	kubectl logs secret-test-pod-cc | grep -q "PROTECTED_SECRET = unsealed_secret"
}

teardown() {
	[ "${KATA_HYPERVISOR}" = "qemu-coco-dev" ] || skip "Test not ready yet for ${KATA_HYPERVISOR}"

	if [ "${KBS}" = "false" ]; then
		skip "Test skipped as KBS not setup"
	fi

	[ -n "${pod_name:-}" ] && kubectl describe "pod/${pod_name}" || true
	[ -n "${pod_config_dir:-}" ] && kubectl delete -f "${K8S_TEST_YAML}" || true

	kubectl delete secret sealed-secret --ignore-not-found
	kubectl delete secret not-sealed-secret --ignore-not-found

	if [[ -n "${node_start_time:-}" && -z "$BATS_TEST_COMPLETED" ]]; then
		echo "DEBUG: system logs of node '$node' since test start time ($node_start_time)"
		print_node_journal "$node" "kata" --since "$node_start_time" || true
	fi
}
