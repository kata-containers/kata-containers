#!/usr/bin/env bash
# Copyright 2026 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

SNP_HYPERVISORS=("qemu-snp" "qemu-snp-runtime-rs")
TDX_HYPERVISORS=("qemu-tdx" "qemu-tdx-runtime-rs")
SE_HYPERVISORS=("qemu-se" "qemu-se-runtime-rs")
CCA_HYPERVISORS=("qemu-cca")
GPU_TEE_HYPERVISORS=("qemu-nvidia-gpu-snp" "qemu-nvidia-gpu-tdx")
TEE_HYPERVISORS=("${SNP_HYPERVISORS[@]}" "${TDX_HYPERVISORS[@]}" "${SE_HYPERVISORS[@]}" "${CCA_HYPERVISORS[@]}" "${GPU_TEE_HYPERVISORS[@]}")
NON_TEE_HYPERVISORS=("qemu-coco-dev" "qemu-coco-dev-runtime-rs")
FIRECRACKER_HYPERVISORS=("firecracker" "fc")

function is_snp_hypervisor() {
	local hypervisor="${1:-${KATA_HYPERVISOR}}"
	# shellcheck disable=SC2076 # intentionally use literal string matching
	[[ " ${SNP_HYPERVISORS[*]} " =~ " ${hypervisor} " ]] && return 0
	return 1
}

function is_tdx_hypervisor() {
	local hypervisor="${1:-${KATA_HYPERVISOR}}"
	# shellcheck disable=SC2076 # intentionally use literal string matching
	[[ " ${TDX_HYPERVISORS[*]} " =~ " ${hypervisor} " ]] && return 0
	return 1
}

function is_se_hypervisor() {
	local hypervisor="${1:-${KATA_HYPERVISOR}}"
	# shellcheck disable=SC2076 # intentionally use literal string matching
	[[ " ${SE_HYPERVISORS[*]} " =~ " ${hypervisor} " ]] && return 0
	return 1
}

function is_cca_hypervisor() {
	local hypervisor="${1:-${KATA_HYPERVISOR}}"
	# shellcheck disable=SC2076 # intentionally use literal string matching
	[[ " ${CCA_HYPERVISORS[*]} " =~ " ${hypervisor} " ]] && return 0
	return 1
}

function is_non_tee_hypervisor() {
	local hypervisor="${1:-${KATA_HYPERVISOR}}"
	# shellcheck disable=SC2076 # intentionally use literal string matching
	[[ " ${NON_TEE_HYPERVISORS[*]} " =~ " ${hypervisor} " ]] && return 0
	return 1
}

function is_confidential_gpu_hypervisor() {
	local hypervisor="${1:-${KATA_HYPERVISOR}}"
	# shellcheck disable=SC2076 # intentionally use literal string matching
	[[ " ${GPU_TEE_HYPERVISORS[*]} " =~ " ${hypervisor} " ]] && return 0
	return 1
}

function is_firecracker_hypervisor() {
	local hypervisor="${1:-${KATA_HYPERVISOR}}"
	# shellcheck disable=SC2076 # intentionally use literal string matching
	[[ " ${FIRECRACKER_HYPERVISORS[*]} " =~ " ${hypervisor} " ]] && return 0
	return 1
}

# Common check for confidential hardware (TEE) runtime class.
function is_confidential_hardware() {
	local hypervisor="${1:-${KATA_HYPERVISOR}}"
	# This check must be done with "<SPACE>${KATA_HYPERVISOR}<SPACE>" to avoid
	# having substrings, like qemu, being matched with qemu-$something.
	# shellcheck disable=SC2076 # intentionally use literal string matching
	if [[ " ${TEE_HYPERVISORS[*]} " =~ " ${hypervisor} " ]]; then
		return 0
	fi
	return 1
}

# Common check for confidential runtime class.
function is_confidential_runtime_class() {
	local hypervisor="${1:-${KATA_HYPERVISOR}}"
	if is_confidential_hardware "${hypervisor}" || is_non_tee_hypervisor "${hypervisor}"; then
		return 0
	else
		return 1
	fi
}

is_hotplug_supported() {
	local hypervisor="${1:-${KATA_HYPERVISOR}}"
	if is_confidential_runtime_class "${hypervisor}"; then
		echo "Confidential computing hypervisors don't support hotplug" >&2
		return 1
	elif is_firecracker_hypervisor "${hypervisor}"; then
		echo "FC doesn't support hotplug" >&2
		return 1
	fi
	return 0
}
