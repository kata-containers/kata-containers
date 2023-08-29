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
	SUPPORTED_HYPERVISORS=("qemu-sev" "qemu-snp")

	# This check must be done with "<SPACE>${KATA_HYPERVISOR}<SPACE>" to avoid
	# having substrings, like qemu, being matched with qemu-$something.
	[[ " ${SUPPORTED_HYPERVISORS[*]} " =~  " ${KATA_HYPERVISOR} " ]] ||  skip "Test not supported for ${KATA_HYPERVISOR}."

	get_pod_config_dir
	setup_unencrypted_confidential_pod
}

@test "Test unencrypted confidential container launch success and verify that we are running in a secure enclave." {
	# Start the service/deployment/pod
	kubectl apply -f "${pod_config_dir}/pod-confidential-unencrypted.yaml"

	# Retrieve pod name, wait for it to come up, retrieve pod ip
	pod_name=$(kubectl get pod -o wide | grep "confidential-unencrypted" | awk '{print $1;}')

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "${pod_name}"

	pod_ip=$(kubectl get pod -o wide | grep "confidential-unencrypted" | awk '{print $6;}')

	# Run the remote command
	coco_enabled=$(ssh -i ${SSH_KEY_FILE} -o "StrictHostKeyChecking no" -o "PasswordAuthentication=no" root@${pod_ip} /bin/sh -c "$(get_remote_command_per_hypervisor)" || true)

	if [ -z "$coco_enabled" ]; then
		>&2 echo -e "Confidential compute is expected but not enabled."
		return 1
	fi
}

teardown() {
	[[ " ${SUPPORTED_HYPERVISORS[*]} " =~  " ${KATA_HYPERVISOR} " ]] ||  skip "Test not supported for ${KATA_HYPERVISOR}."

	kubectl describe "pod/${pod_name}" || true
	kubectl delete -f "${pod_config_dir}/pod-confidential-unencrypted.yaml" || true
}
