#!/bin/bash
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Mount a kata extension block device with optional dm-verity verification.
# Usage: kata-extension-mount.sh <extension-name>
#
# The block device is discovered by scanning /sys/block/*/serial for
# a device whose serial matches "extension-<name>".
# Verity params are read from kernel cmdline: kata.extension.<name>.verity_params=...
# The extension is mounted read-only (erofs) at /run/kata-extensions/<name>/.

set -euo pipefail

EXTENSION_NAME="${1:?extension name required}"
SERIAL="extension-${EXTENSION_NAME}"
MOUNT_DIR="/run/kata-extensions/${EXTENSION_NAME}"

find_block_dev_by_serial() {
	local wanted="$1"
	for s in /sys/block/*/serial; do
		[[ -f "${s}" ]] || continue
		local cur
		cur="$(cat "${s}" 2>/dev/null)" || continue
		if [[ "${cur}" == "${wanted}" ]]; then
			echo "/dev/$(basename "$(dirname "${s}")")"
			return 0
		fi
	done
	return 1
}

REAL_DEV="$(find_block_dev_by_serial "${SERIAL}")" || {
	echo "ERROR: no block device with serial ${SERIAL} found" >&2
	exit 1
}

get_verity_param() {
	local key="kata.extension.${EXTENSION_NAME}.verity_params"
	local cmdline
	cmdline="$(cat /proc/cmdline)"

	local value=""
	for param in ${cmdline}; do
		case "${param}" in
			"${key}="*)
				value="${param#"${key}="}"
				;;
		esac
	done
	echo "${value}"
}

parse_verity_field() {
	local params="$1"
	local field="$2"
	# Iterate without a pipeline: a `| while` loop runs in a subshell, so a
	# `return` there would only exit the subshell, never this function.
	local IFS=','
	for pair in ${params}; do
		local k="${pair%%=*}"
		local v="${pair#*=}"
		if [[ "${k}" == "${field}" ]]; then
			echo "${v}"
			return
		fi
	done
}

VERITY_PARAMS="$(get_verity_param)"

PART_SEP=""
[[ "${REAL_DEV}" =~ [0-9]$ ]] && PART_SEP="p"
DATA_DEV="${REAL_DEV}${PART_SEP}1"
HASH_DEV="${REAL_DEV}${PART_SEP}2"

# The image build encodes its integrity policy in the on-disk layout, and that
# layout -- not the kernel command line -- is the source of truth for whether
# this extension must be verified: a measured extension (MEASURED_ROOTFS=yes)
# is built with a dm-verity hash partition (p2) next to the data partition (p1);
# an unmeasured extension (MEASURED_ROOTFS=no, e.g. on s390x, where Secure
# Execution protects the guest through a different mechanism) has only p1.
#
# We cross-check that layout against the verity params on the kernel command
# line (which, in a confidential guest, is part of the measured/attested boot)
# so we fail closed instead of silently downgrading a measured extension to an
# unverified mount:
#
#   hash device + params       -> verify (normal measured extension)
#   hash device + NO params    -> refuse: verity was stripped/disabled (tamper)
#   NO hash device + params    -> refuse: params but nothing to verify (mismatch)
#   NO hash device + NO params -> raw mount (genuinely unmeasured extension)
#
# See "Integrity policy: measured vs. unmeasured, and failing closed" in
# docs/design/composable-vm-images.md for the rationale.
if [[ -b "${HASH_DEV}" ]]; then
	if [[ -z "${VERITY_PARAMS}" ]]; then
		echo "ERROR: extension ${EXTENSION_NAME} ships a dm-verity hash device but no verity params were provided on the kernel command line; refusing to mount it unverified" >&2
		exit 1
	fi

	ROOT_HASH="$(parse_verity_field "${VERITY_PARAMS}" "root_hash")"
	SALT="$(parse_verity_field "${VERITY_PARAMS}" "salt")"
	DATA_BLOCKS="$(parse_verity_field "${VERITY_PARAMS}" "data_blocks")"
	HASH_BLOCK_SIZE="$(parse_verity_field "${VERITY_PARAMS}" "hash_block_size")"
	DATA_BLOCK_SIZE="$(parse_verity_field "${VERITY_PARAMS}" "data_block_size")"

	if [[ -z "${ROOT_HASH}" ]]; then
		echo "ERROR: extension ${EXTENSION_NAME} verity params carry no root_hash; refusing to mount" >&2
		exit 1
	fi

	DM_NAME="extension-${EXTENSION_NAME}"

	veritysetup open "${DATA_DEV}" "${DM_NAME}" "${HASH_DEV}" "${ROOT_HASH}" \
		--no-superblock \
		--hash-block-size="${HASH_BLOCK_SIZE:-4096}" \
		--data-block-size="${DATA_BLOCK_SIZE:-4096}" \
		--data-blocks="${DATA_BLOCKS}" \
		--salt="${SALT}"

	MOUNT_SRC="/dev/mapper/${DM_NAME}"
else
	if [[ -n "${VERITY_PARAMS}" ]]; then
		echo "ERROR: extension ${EXTENSION_NAME} has verity params on the kernel command line but no dm-verity hash device; refusing to mount" >&2
		exit 1
	fi

	if [[ -b "${DATA_DEV}" ]]; then
		MOUNT_SRC="${DATA_DEV}"
	else
		MOUNT_SRC="${REAL_DEV}"
	fi
fi

mkdir -p "${MOUNT_DIR}"
mount -t erofs -o ro "${MOUNT_SRC}" "${MOUNT_DIR}"

echo "Extension ${EXTENSION_NAME} mounted at ${MOUNT_DIR}"
