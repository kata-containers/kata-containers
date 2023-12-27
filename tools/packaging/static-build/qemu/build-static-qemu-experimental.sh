#!/usr/bin/env bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "${script_dir}/../../scripts/lib.sh"

qemu_repo="${qemu_repo:-}"
qemu_version="${qemu_version:-}"
qemu_suffix="${qemu_suffix:-}"
qemu_tarball_name="${qemu_tarball_name:-}"

[ -n "$qemu_repo" ] || die "failed to get qemu repo"
[ -n "$qemu_version" ] || die "failed to get qemu version"
[ -n "$qemu_suffix" ] || die "failed to get qemu suffix"
[ -n "$qemu_tarball_name" ] || die "failed to get qemu tarball name"

"${script_dir}/build-base-qemu.sh" "${qemu_repo}" "${qemu_version}" "${qemu_suffix}" "${qemu_tarball_name}"
