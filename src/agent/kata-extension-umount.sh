#!/bin/bash
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Unmount a kata extension block device.
# Usage: kata-extension-umount.sh <extension-name>

set -euo pipefail

EXTENSION_NAME="${1:?extension name required}"
MOUNT_DIR="/run/kata-extensions/${EXTENSION_NAME}"
DM_NAME="extension-${EXTENSION_NAME}"

# The extension is consumed in place (the agent reads binaries/config straight from
# ${MOUNT_DIR} via the component manifest), so there are no bind mounts to undo
# here -- we only need to unmount the extension filesystem and close dm-verity.

# Unmount the extension filesystem
if mountpoint -q "${MOUNT_DIR}" 2>/dev/null; then
	umount "${MOUNT_DIR}"
fi

# Close dm-verity device if present
if [[ -e "/dev/mapper/${DM_NAME}" ]]; then
	veritysetup close "${DM_NAME}" 2>/dev/null || true
fi

echo "Extension ${EXTENSION_NAME} unmounted"
