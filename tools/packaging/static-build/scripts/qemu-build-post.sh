#!/usr/bin/env bash
#
# Copyright (c) 2020 Red Hat, Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# This script process QEMU post-build.
#
set -e

script_dir="$(realpath "$(dirname "$0")")"
# shellcheck source=/dev/null
source "${script_dir}/../qemu.blacklist"

if [[ -z "${QEMU_TARBALL}" || -z "${QEMU_DESTDIR}" ]]; then
	echo "$0: needs QEMU_TARBALL and QEMU_DESTDIR exported"
	exit 1
fi

pushd "${QEMU_DESTDIR}"
# shellcheck disable=SC2154
for pattern in "${qemu_black_list[@]}"; do
	find . -path "${pattern}" -print0 | xargs -0 --no-run-if-empty rm -rfv
done

if [[ -n "${BUILD_SUFFIX}" ]]; then
	echo "Rename binaries using ${BUILD_SUFFIX}"
	# shellcheck disable=SC2154,SC1083,SC2086
	find . -name 'qemu-system-*' -exec mv {} {}-${BUILD_SUFFIX} \;
	# shellcheck disable=SC2154
	if [[ "${ARCH}" != "x86_64" ]]; then
		# shellcheck disable=SC1083,SC2086
		find . -name 'virtiofsd' -exec mv {} {}-${BUILD_SUFFIX} \;
	fi
fi

echo "INFO: create the tarball"
tar -czvf "${QEMU_TARBALL}" ./*
popd
