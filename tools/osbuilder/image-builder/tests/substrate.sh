#!/usr/bin/env bash
# Copyright (c) 2026 NVIDIA Corporation
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

builder=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/image_builder.sh
work=$(mktemp -d)
trap 'rm -rf "${work}"' EXIT
rootfs="${work}/rootfs"
mkdir -p "${rootfs}/usr/bin" "${rootfs}/usr/sbin"
ln -s usr/sbin "${rootfs}/sbin"
printf '#!/bin/sh\nexit 0\n' > "${rootfs}/usr/bin/kata-agent"
chmod +x "${rootfs}/usr/bin/kata-agent"
ln -s ../bin/kata-agent "${rootfs}/usr/sbin/init"
printf 'guest payload\n' > "${rootfs}/payload"
export BUILD_VARIANT=substrate AGENT_INIT=yes SOURCE_DATE_EPOCH=1700000000

bash "${builder}" -o "${work}/first.img" "${rootfs}"
dd if=/dev/zero bs=1M count=$(( $(stat -c %s "${work}/first.img") / 1048576 + 1 )) status=none |
	tr '\000' '\377' > "${work}/second.img"
bash "${builder}" -o "${work}/second.img" "${rootfs}"
cmp "${work}/first.img" "${work}/second.img"
[[ $(( $(stat -c %s "${work}/first.img") % 2097152 )) == 0 ]]
[[ $(sfdisk --json "${work}/first.img" | jq '.partitiontable.partitions[0].start') == 4096 ]]
dd if="${work}/first.img" of="${work}/part.ext4" bs=512 skip=4096 status=none
e2fsck -fn "${work}/part.ext4"
[[ $(debugfs -R 'cat /payload' "${work}/part.ext4" 2>/dev/null) == 'guest payload' ]]
tune2fs -l "${work}/part.ext4" > "${work}/superblock"
if grep '^Filesystem features:' "${work}/superblock" | grep -q has_journal; then
	echo 'Read-only image must not have a journal' >&2
	exit 1
fi
grep '^Default mount options:' "${work}/superblock" | grep -q journal_data_ordered
if MEASURED_ROOTFS=yes bash "${builder}" -o "${work}/measured.img" "${rootfs}" > "${work}/rejected.log" 2>&1; then
	echo 'Measured rootfs must be rejected' >&2
	exit 1
fi
grep -q 'Substrate does not support SELinux or measured rootfs images' "${work}/rejected.log"
echo 'Substrate userspace ext4 checks passed'
