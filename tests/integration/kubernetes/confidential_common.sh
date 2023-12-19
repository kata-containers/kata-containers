#!/usr/bin/env bash
# Copyright 2022-2023 Advanced Micro Devices, Inc.
# Copyright 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

source "${BATS_TEST_DIRNAME}/tests_common.sh"

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

	echo "${REMOTE_COMMAND_PER_HYPERVISOR[${KATA_HYPERVISOR}]}"
}
