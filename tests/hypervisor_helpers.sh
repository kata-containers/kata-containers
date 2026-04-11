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

function is_snp_hypervisor() {
	local hypervisor="$1"
	# shellcheck disable=SC2076 # intentionally use literal string matching
	[[ " ${SNP_HYPERVISORS[*]} " =~ " ${hypervisor} " ]] && return 0
	return 1
}

function is_tdx_hypervisor() {
	local hypervisor="$1"
	# shellcheck disable=SC2076 # intentionally use literal string matching
	[[ " ${TDX_HYPERVISORS[*]} " =~ " ${hypervisor} " ]] && return 0
	return 1
}

function is_se_hypervisor() {
	local hypervisor="$1"
	# shellcheck disable=SC2076 # intentionally use literal string matching
	[[ " ${SE_HYPERVISORS[*]} " =~ " ${hypervisor} " ]] && return 0
	return 1
}

function is_cca_hypervisor() {
	local hypervisor="$1"
	# shellcheck disable=SC2076 # intentionally use literal string matching
	[[ " ${CCA_HYPERVISORS[*]} " =~ " ${hypervisor} " ]] && return 0
	return 1
}

function is_non_tee_hypervisor() {
	local hypervisor="$1"
	# shellcheck disable=SC2076 # intentionally use literal string matching
	[[ " ${NON_TEE_HYPERVISORS[*]} " =~ " ${hypervisor} " ]] && return 0
	return 1
}

function is_confidential_gpu_hypervisor() {
	local hypervisor="$1"
	# shellcheck disable=SC2076 # intentionally use literal string matching
	[[ " ${GPU_TEE_HYPERVISORS[*]} " =~ " ${hypervisor} " ]] && return 0
	return 1
}

# Common check for confidential hardware (e.g. not-non-tee) runtime class.
function is_confidential_hardware() {
	local kata_hypervisor="${1}"
	# This check must be done with "<SPACE>${KATA_HYPERVISOR}<SPACE>" to avoid
	# having substrings, like qemu, being matched with qemu-$something.
	# shellcheck disable=SC2076 # intentionally use literal string matching
	if [[ " ${TEE_HYPERVISORS[*]} " =~ " ${kata_hypervisor} " ]]; then
		return 0
	fi
	return 1
}

# Common check for confidential runtime class.
function is_confidential_runtime_class() {
	local kata_hypervisor="${1}"
	if is_confidential_hardware "${kata_hypervisor}" || is_non_tee_hypervisor "${kata_hypervisor}"; then
		return 0
	else
		return 1
	fi
}
