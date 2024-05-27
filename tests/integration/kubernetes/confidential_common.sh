#!/usr/bin/env bash
# Copyright 2022-2023 Advanced Micro Devices, Inc.
# Copyright 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

source "${BATS_TEST_DIRNAME}/tests_common.sh"
source "${BATS_TEST_DIRNAME}/../../common.bash"

SUPPORTED_TEE_HYPERVISORS=("qemu-sev" "qemu-snp" "qemu-tdx" "qemu-se")
SUPPORTED_NON_TEE_HYPERVISORS=("qemu-coco-dev")

function setup_unencrypted_confidential_pod() {
	get_pod_config_dir

	export SSH_KEY_FILE="${pod_config_dir}/confidential/unencrypted/ssh/unencrypted"

	if [ -n "${PR_NUMBER}" ]; then
		# Use correct address in pod yaml
		sed -i "s/-nightly/-${PR_NUMBER}/" "${pod_config_dir}/pod-confidential-unencrypted.yaml"
	fi

	# Set permissions on private key file
	sudo chmod 600 "${SSH_KEY_FILE}"
}

# This function relies on `KATA_HYPERVISOR` being an environment variable
# and returns the remote command to be executed to that specific hypervisor
# in order to identify whether the workload is running on a TEE environment
function get_remote_command_per_hypervisor() {
	declare -A REMOTE_COMMAND_PER_HYPERVISOR
	REMOTE_COMMAND_PER_HYPERVISOR[qemu-sev]="dmesg | grep \"Memory Encryption Features active:.*\(SEV$\|SEV \)\""
	REMOTE_COMMAND_PER_HYPERVISOR[qemu-snp]="dmesg | grep \"Memory Encryption Features active:.*SEV-SNP\""
	REMOTE_COMMAND_PER_HYPERVISOR[qemu-tdx]="cpuid | grep TDX_GUEST"
	REMOTE_COMMAND_PER_HYPERVISOR[qemu-se]="cd /sys/firmware/uv; cat prot_virt_guest | grep 1"

	echo "${REMOTE_COMMAND_PER_HYPERVISOR[${KATA_HYPERVISOR}]}"
}

# This function verifies whether the input hypervisor supports confidential tests and
# relies on `KATA_HYPERVISOR` being an environment variable
function check_hypervisor_for_confidential_tests() {
	local kata_hypervisor="${1}"
	# This check must be done with "<SPACE>${KATA_HYPERVISOR}<SPACE>" to avoid
	# having substrings, like qemu, being matched with qemu-$something.
	if check_hypervisor_for_confidential_tests_tee_only "${kata_hypervisor}" ||\
	[[ " ${SUPPORTED_NON_TEE_HYPERVISORS[*]} " =~ " ${kata_hypervisor} " ]]; then
		return 0
	else
		return 1
	fi
}

# This function verifies whether the input hypervisor supports confidential tests and
# relies on `KATA_HYPERVISOR` being an environment variable
function check_hypervisor_for_confidential_tests_tee_only() {
	local kata_hypervisor="${1}"
	# This check must be done with "<SPACE>${KATA_HYPERVISOR}<SPACE>" to avoid
	# having substrings, like qemu, being matched with qemu-$something.
	if [[ " ${SUPPORTED_TEE_HYPERVISORS[*]} " =~ " ${kata_hypervisor} " ]]; then
		return 0
	fi

	return 1
}

# Common check for confidential tests.
function is_confidential_runtime_class() {
	if check_hypervisor_for_confidential_tests "${KATA_HYPERVISOR}"; then
		return 0
	fi

	return 1
}

# Common check for confidential hardware tests.
function is_confidential_hardware() {
	if check_hypervisor_for_confidential_tests_tee_only "${KATA_HYPERVISOR}"; then
		return 0
	fi

	return 1
}
