#!/usr/bin/env bash
# Copyright 2022-2023 Advanced Micro Devices, Inc.
# Copyright 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

source "${BATS_TEST_DIRNAME}/tests_common.sh"
source "${BATS_TEST_DIRNAME}/../../common.bash"

load "${BATS_TEST_DIRNAME}/confidential_kbs.sh"

SUPPORTED_TEE_HYPERVISORS=("qemu-sev" "qemu-snp" "qemu-tdx" "qemu-se")
SUPPORTED_NON_TEE_HYPERVISORS=("qemu-coco-dev")

function setup_unencrypted_confidential_pod() {
	get_pod_config_dir

	export SSH_KEY_FILE="${pod_config_dir}/confidential/unencrypted/ssh/unencrypted"

	if [ -n "${GH_PR_NUMBER}" ]; then
		# Use correct address in pod yaml
		sed -i "s/-nightly/-${GH_PR_NUMBER}/" "${pod_config_dir}/pod-confidential-unencrypted.yaml"
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

function create_loop_device(){
	local loop_file="${1:-/tmp/trusted-image-storage.img}"
	local node="$(get_one_kata_node)"
	cleanup_loop_device "$loop_file"

	exec_host "$node" "dd if=/dev/zero of=$loop_file bs=1M count=2500"
	exec_host "$node" "losetup -fP $loop_file >/dev/null 2>&1"
	local device=$(exec_host "$node" losetup -j $loop_file | awk -F'[: ]' '{print $1}')

	echo $device
}

function cleanup_loop_device(){
	local loop_file="${1:-/tmp/trusted-image-storage.img}"
	local node="$(get_one_kata_node)"
	# Find all loop devices associated with $loop_file
	local existed_devices=$(exec_host "$node" losetup -j $loop_file | awk -F'[: ]' '{print $1}')

	if [ -n "$existed_devices" ]; then
		# Iterate over each found loop device and detach it
		for d in $existed_devices; do
			exec_host "$node" "losetup -d "$d" >/dev/null 2>&1"
		done
	fi

	exec_host "$node" "rm -f "$loop_file" >/dev/null 2>&1 || true"
}

# This function creates pod yaml. Parameters
# - $1: image reference
# - $2: image policy file. If given, `enable_signature_verification` will be set to true
# - $3: image registry auth.
# - $4: guest components procs parameter
# - $5: guest components rest api parameter
# - $6: node
function create_coco_pod_yaml() {
	image=$1
	image_policy=${2:-}
	image_registry_auth=${3:-}
	guest_components_procs=${4:-}
	guest_components_rest_api=${5:-}
	node=${6:-}

	local CC_KBS_ADDR
	export CC_KBS_ADDR=$(kbs_k8s_svc_http_addr)

	kernel_params_annotation="io.katacontainers.config.hypervisor.kernel_params"
	kernel_params_value=""

	if [ -n "$image_policy" ]; then
		kernel_params_value+=" agent.image_policy_file=${image_policy}"
		kernel_params_value+=" agent.enable_signature_verification=true"
	fi

	if [ -n "$image_registry_auth" ]; then
		kernel_params_value+=" agent.image_registry_auth=${image_registry_auth}"
	fi

	if [ -n "$guest_components_procs" ]; then
		kernel_params_value+=" agent.guest_components_procs=${guest_components_procs}"
	fi

	if [ -n "$guest_components_rest_api" ]; then
		kernel_params_value+=" agent.guest_components_rest_api=${guest_components_rest_api}"
	fi

	kernel_params_value+=" agent.aa_kbc_params=cc_kbc::${CC_KBS_ADDR}"

	# Note: this is not local as we use it in the caller test
	kata_pod="$(new_pod_config "$image" "kata-${KATA_HYPERVISOR}")"
	set_container_command "${kata_pod}" "0" "sleep" "30"

	# Set annotations
	set_metadata_annotation "${kata_pod}" \
		"io.containerd.cri.runtime-handler" \
		"kata-${KATA_HYPERVISOR}"
	set_metadata_annotation "${kata_pod}" \
		"${kernel_params_annotation}" \
		"${kernel_params_value}"

	add_allow_all_policy_to_yaml "${kata_pod}"

	if [ -n "$node" ]; then
		set_node "${kata_pod}" "$node"
	fi
}
