#!/bin/bash
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Unmount a kata addon block device.
# Usage: kata-addon-umount.sh <addon-name>

set -euo pipefail

ADDON_NAME="${1:?addon name required}"
MOUNT_DIR="/run/kata-addons/${ADDON_NAME}"
DM_NAME="addon-${ADDON_NAME}"

# Undo bind mounts from /usr/local/bin/
if [[ -d "${MOUNT_DIR}/usr/local/bin" ]]; then
	for bin in "${MOUNT_DIR}"/usr/local/bin/*; do
		[[ -f "${bin}" ]] || continue
		target="/usr/local/bin/$(basename "${bin}")"
		umount "${target}" 2>/dev/null || true
	done
fi

# Undo bind mounts from /etc/
if [[ -d "${MOUNT_DIR}/etc" ]]; then
	for cfg in "${MOUNT_DIR}"/etc/*; do
		[[ -e "${cfg}" ]] || continue
		target="/etc/$(basename "${cfg}")"
		umount "${target}" 2>/dev/null || true
	done
fi

# Undo pause_bundle bind mount
if mountpoint -q /pause_bundle 2>/dev/null; then
	umount /pause_bundle 2>/dev/null || true
fi

# Unmount the addon filesystem
if mountpoint -q "${MOUNT_DIR}" 2>/dev/null; then
	umount "${MOUNT_DIR}"
fi

# Close dm-verity device if present
if [[ -e "/dev/mapper/${DM_NAME}" ]]; then
	veritysetup close "${DM_NAME}" 2>/dev/null || true
fi

echo "Addon ${ADDON_NAME} unmounted"
