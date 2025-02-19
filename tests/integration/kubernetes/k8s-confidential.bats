#!/usr/bin/env bats
# Copyright 2022-2023 Advanced Micro Devices, Inc.
# Copyright 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	if ! is_confidential_hardware; then
		skip "Test is supported only on confidential hardware (which ${KATA_HYPERVISOR} is not)"
	fi
	setup_unencrypted_confidential_pod
}

@test "Test unencrypted confidential container launch success and verify that we are running in a secure enclave." {
	[[ " ${SUPPORTED_NON_TEE_HYPERVISORS} " =~ " ${KATA_HYPERVISOR} " ]] && skip "Test not supported for ${KATA_HYPERVISOR}."
	# Start the service/deployment/pod
	kubectl apply -f "${pod_config_dir}/pod-confidential-unencrypted.yaml"

	# Retrieve pod name, wait for it to come up, retrieve pod ip
	pod_name=$(kubectl get pod -o wide | grep "confidential-unencrypted" | awk '{print $1;}')

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "${pod_name}"

	coco_enabled=""
	for i in {1..6}; do
		rm -f "${HOME}/.ssh/known_hosts"
		if ! pod_ip=$(kubectl get pod -o wide | grep "confidential-unencrypted" | awk '{print $6;}'); then
			warn "Failed to get pod IP address."
		else
			info "Pod IP address: ${pod_ip}"
			coco_enabled=$(ssh -i ${SSH_KEY_FILE} -o "StrictHostKeyChecking no" -o "PasswordAuthentication=no" root@${pod_ip} "$(get_remote_command_per_hypervisor)" 2> /dev/null) && break
			warn "Failed to connect to pod."
		fi
		sleep 5
	done
	[ -z "$coco_enabled" ] && die "Confidential compute is expected but not enabled."
	info "ssh client output: ${coco_enabled}"
}

teardown() {
	if ! is_confidential_hardware; then
		skip "Test is supported only on confidential hardware (which ${KATA_HYPERVISOR} is not)"
	fi
	
	kubectl describe "pod/${pod_name}" || true
	kubectl delete -f "${pod_config_dir}/pod-confidential-unencrypted.yaml" || true
}
