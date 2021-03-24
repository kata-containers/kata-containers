#!/bin/bash
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

if [ -z "$qemu_repo" ]; then
	info "Get qemu information from runtime versions.yaml"
	qemu_url=$(get_from_kata_deps "assets.hypervisor.qemu-experimental.url")
	[ -n "$qemu_url" ] || die "failed to get qemu url"
	qemu_repo="${qemu_url}.git"
fi
[ -n "$qemu_repo" ] || die "failed to get qemu repo"

[ -n "$qemu_version" ] || qemu_version=$(get_from_kata_deps "assets.hypervisor.qemu-experimental.version")
[ -n "$qemu_version" ] || die "failed to get qemu version"

"${script_dir}/build-base-qemu.sh" "${qemu_repo}" "${qemu_version}" "-experimental" "kata-static-qemu-experimental.tar.gz"
