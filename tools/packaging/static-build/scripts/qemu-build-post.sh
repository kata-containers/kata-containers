#!/bin/bash
#
# Copyright (c) 2020 Red Hat, Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# This script process QEMU post-build.
#
set -e

script_dir="$(realpath $(dirname $0))"
source "${script_dir}/../qemu.blacklist"

if [[ -z "${QEMU_TARBALL}" || -z "${QEMU_DESTDIR}" ]]; then
	echo "$0: needs QEMU_TARBALL and QEMU_DESTDIR exported"
	exit 1
fi

pushd "${QEMU_DESTDIR}"
# Remove files to reduce the surface.
echo "INFO: remove uneeded files"
for pattern in ${qemu_black_list[@]}; do
	find . -path "$pattern" | xargs rm -rfv
done

if [[ -n "${BUILD_SUFFIX}" ]]; then
	echo "Rename binaries using $BUILD_SUFFIX"
	find -name 'qemu-system-*' -exec mv {} {}-experimental \;
	find -name 'virtiofsd' -exec mv {} {}-experimental \;
fi

echo "INFO: create the tarball"
tar -czvf "${QEMU_TARBALL}" *
popd
