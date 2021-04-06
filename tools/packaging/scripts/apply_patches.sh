#!/bin/bash
#
# Copyright (c) 2020 Red Hat, Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# This script apply patches.
#
set -e

script_dir="$(realpath $(dirname $0))"
patches_dir="$1"

if [ -z "$patches_dir" ]; then
	cat <<-EOT
	Apply patches to the sources at the current directory.

	Patches are expected to be named in the standard git-format-patch(1) format where
	the first part of the filename represents the patch ordering (lowest numbers
	apply first):
	    'NUMBER-DASHED_DESCRIPTION.patch'

	For example,

	    0001-fix-the-bad-thing.patch
	    0002-improve-the-fix-the-bad-thing-fix.patch
	    0003-correct-compiler-warnings.patch

	Usage:
	    $0 PATCHES_DIR
	Where:
	    PATCHES_DIR is the directory containing the patches
	EOT
	exit 1
fi

echo "INFO: Apply patches from $patches_dir"
if [ -d "$patches_dir" ]; then
	patches=($(find "$patches_dir" -name '*.patch'|sort -t- -k1,1n))
	echo "INFO: Found ${#patches[@]} patches"
	for patch in ${patches[@]}; do
		echo "INFO: Apply $patch"
		git apply "$patch" || \
			{ echo >&2 "ERROR: Not applied. Exiting..."; exit 1; }
	done
else
	echo "INFO: Patches directory does not exist: ${patches_dir}"
	echo "INFO: Create a ${patches_dir}/no_patches file if the current version has no patches"
	exit 1;
fi
