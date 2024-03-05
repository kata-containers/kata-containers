#!/usr/bin/env bash
#
# Copyright (c) 2024 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

# shellcheck disable=SC2034,SC2154

# this is where the kernel-uvm package installation places bzImage, see SPEC file
BZIMAGE_BIN="/usr/share/cloud-hypervisor/bzImage"

IGVM_EXTRACT_FOLDER="${SCRIPT_DIR}/igvm-tooling"
CLH_ACPI_TABLES_DIR="${IGVM_EXTRACT_FOLDER}/src/igvm/acpi/acpi-clh/"
IGVM_PY_FILE="${IGVM_EXTRACT_FOLDER}/src/igvm/igvmgen.py"

IGVM_BUILD_VARS=(
	"-kernel" "${BZIMAGE_BIN}"
	"-boot_mode" "x64"
	"-vtl" "0"
	"-svme" "1"
	"-encrypted_page" "1"
	"-pvalidate_opt" "1"
	"-acpi" "${CLH_ACPI_TABLES_DIR}"
)

IGVM_KERNEL_PARAMS_COMMON="dm-mod.create=\"dm-verity,,,ro,0 ${IMAGE_DATA_SECTORS} verity 1 /dev/vda1 /dev/vda2 ${IMAGE_DATA_BLOCK_SIZE} ${IMAGE_HASH_BLOCK_SIZE} ${IMAGE_DATA_BLOCKS} 0 sha256 ${IMAGE_ROOT_HASH} ${IMAGE_SALT}\" \
	root=/dev/dm-0 rootflags=data=ordered,errors=remount-ro ro rootfstype=ext4 panic=1 no_timer_check noreplace-smp systemd.unit=kata-containers.target systemd.mask=systemd-networkd.service \
	systemd.mask=systemd-networkd.socket agent.enable_signature_verification=false"
IGVM_KERNEL_PROD_PARAMS="${IGVM_KERNEL_PARAMS_COMMON} quiet"
IGVM_KERNEL_DEBUG_PARAMS="${IGVM_KERNEL_PARAMS_COMMON} console=hvc0 systemd.log_target=console agent.log=debug agent.debug_console agent.debug_console_vport=1026"

IGVM_FILE_NAME="kata-containers-igvm.img"
IGVM_DBG_FILE_NAME="kata-containers-igvm-debug.img"
IGVM_MEASUREMENT_FILE_NAME="igvm-measurement.cose"
IGVM_DBG_MEASUREMENT_FILE_NAME="igvm-debug-measurement.cose"
