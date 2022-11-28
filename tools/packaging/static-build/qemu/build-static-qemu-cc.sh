#!/usr/bin/env bash
#
# Copyright (c) 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "${script_dir}/../../scripts/lib.sh"

qemu_repo="${qemu_repo:-}"
qemu_version="${qemu_version:-}"
tee="${tee:-}"

export prefix="/opt/confidential-containers/"

if [ -z "${qemu_repo}" ]; then
	info "Get qemu information from runtime versions.yaml"
	export qemu_url=$(get_from_kata_deps "assets.hypervisor.qemu.url")
	[ -n "${qemu_url}" ] || die "failed to get qemu url"
	export qemu_repo="${qemu_url}.git"
fi

[ -n "${qemu_repo}" ] || die "failed to get qemu repo"
[ -n "${qemu_version}" ] || export qemu_version=$(get_from_kata_deps "assets.hypervisor.qemu.version")
[ -n "${qemu_version}" ] || die "failed to get qemu version"

qemu_tarball_name="kata-static-qemu-cc.tar.gz"
[ -n "${tee}" ] && qemu_tarball_name="kata-static-${tee}-qemu-cc.tar.gz"
"${script_dir}/build-base-qemu.sh" "${qemu_repo}" "${qemu_version}" "${tee}" "${qemu_tarball_name}"
