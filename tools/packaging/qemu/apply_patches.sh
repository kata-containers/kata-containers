#!/bin/bash
#
# Copyright (c) 2020 Red Hat, Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# This script apply the needed for Kata Containers patches on QEMU.
# Note: It should be executed from inside the QEMU source directory.
#
set -e

script_dir="$(realpath $(dirname $0))"

qemu_version="$(cat VERSION)"
stable_branch=$(echo $qemu_version | \
	awk 'BEGIN{FS=OFS="."}{print $1 "." $2 ".x"}')
patches_dir="${script_dir}/patches/${stable_branch}"

echo "Handle patches for QEMU $qemu_version (stable ${stable_branch})"
if [ -d $patches_dir ]; then
	for patch in $(find $patches_dir -name '*.patch'); do
		echo "Apply $patch"
		git apply "$patch"
	done
else
	echo "No patches to apply"
fi
