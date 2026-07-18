#!/usr/bin/env bash
# Copyright 2026 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

SNP_HYPERVISORS=("qemu-snp" "qemu-snp-runtime-rs")
TDX_HYPERVISORS=("qemu-tdx" "qemu-tdx-runtime-rs")
SE_HYPERVISORS=("qemu-se" "qemu-se-runtime-rs")
CCA_HYPERVISORS=("qemu-cca")
GPU_TEE_HYPERVISORS=("qemu-nvidia-gpu-snp" "qemu-nvidia-gpu-tdx" "qemu-nvidia-gpu-snp-runtime-rs" "qemu-nvidia-gpu-tdx-runtime-rs")
TEE_HYPERVISORS=("${SNP_HYPERVISORS[@]}" "${TDX_HYPERVISORS[@]}" "${SE_HYPERVISORS[@]}" "${CCA_HYPERVISORS[@]}" "${GPU_TEE_HYPERVISORS[@]}")
NON_TEE_HYPERVISORS=("qemu-coco-dev" "qemu-coco-dev-runtime-rs")
FIRECRACKER_HYPERVISORS=("firecracker" "fc")
# CPU-only NVIDIA classes: boot the verity-backed nvidia base image, no GPU.
NVIDIA_CPU_HYPERVISORS=("qemu-nvidia-cpu" "qemu-nvidia-cpu-runtime-rs")
# All non-confidential NVIDIA classes (CPU-only + plain GPU passthrough).
NVIDIA_HYPERVISORS=("${NVIDIA_CPU_HYPERVISORS[@]}" "qemu-nvidia-gpu" "qemu-nvidia-gpu-runtime-rs")

ALL_HYPERVISORS=(
	"clh"
	"clh-azure"
	"clh-runtime-rs"
	"clh-azure-runtime-rs"
	"dragonball"
	"qemu"
	"qemu-runtime-rs"
	"qemu-nvidia-cpu"
	"qemu-nvidia-cpu-runtime-rs"
	"qemu-nvidia-gpu"
	"qemu-nvidia-gpu-runtime-rs"
	"${TEE_HYPERVISORS[@]}"
	"${NON_TEE_HYPERVISORS[@]}"
	"${FIRECRACKER_HYPERVISORS[@]}"
)

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

# Common check for the non-confidential NVIDIA runtime classes (CPU-only and
# plain GPU passthrough).  The confidential GPU classes are matched by
# is_confidential_gpu_hypervisor instead.
function is_nvidia_hypervisor() {
	local hypervisor="${1:-${KATA_HYPERVISOR}}"
	# shellcheck disable=SC2076 # intentionally use literal string matching
	[[ " ${NVIDIA_HYPERVISORS[*]} " =~ " ${hypervisor} " ]] && return 0
	return 1
}

function is_supported_hypervisor() {
	local hypervisor="${1:-${KATA_HYPERVISOR}}"
	# shellcheck disable=SC2076 # intentionally use literal string matching
	[[ " ${ALL_HYPERVISORS[*]} " =~ " ${hypervisor} " ]] && return 0
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

# Runtime classes that boot with shared_fs=none, where the host cannot read
# guest files directly. Data such as a container's termination log must then be
# retrieved over the agent GetDiagnosticData RPC instead of a shared filesystem.
# This covers the confidential runtime classes and the non-confidential NVIDIA
# CPU runtime-rs handler. The plain qemu-nvidia-cpu (Go) class still uses
# virtio-fs, so it is intentionally excluded.
function is_shared_fs_none_runtime_class() {
	local hypervisor="${1:-${KATA_HYPERVISOR}}"
	if is_confidential_runtime_class "${hypervisor}"; then
		return 0
	fi
	[[ "${hypervisor}" == "qemu-nvidia-cpu-runtime-rs" ]] && return 0
	return 1
}

# Runtime classes that boot a measured (dm-verity) rootfs: the confidential
# classes plus the CPU-only NVIDIA classes, which boot the verity-backed
# nvidia base image without being confidential.
function is_verity_enabled_runtime_class() {
	local hypervisor="${1:-${KATA_HYPERVISOR}}"
	if is_confidential_runtime_class "${hypervisor}"; then
		return 0
	fi
	# shellcheck disable=SC2076 # intentionally use literal string matching
	[[ " ${NVIDIA_CPU_HYPERVISORS[*]} " =~ " ${hypervisor} " ]] && return 0
	return 1
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
