#!/bin/bash
# Copyright (c) 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "${script_dir}/../scripts/lib.sh"
source "${script_dir}/qemu/build-static-qemu-cc.sh"

export KATA_BUILD_CC="${KATA_BUILD_CC:-}"
export qemu_cc_tarball_name="kata-static-qemu-cc.tar.gz"

cache_qemu_artifacts() {
	local current_qemu_version=$(get_from_kata_deps "assets.hypervisor.qemu.version")
	create_qemu_cache_asset "${qemu_cc_tarball_name}" "${current_qemu_version}"
	local qemu_sha=$(calc_qemu_files_sha256sum)
        echo "${current_qemu_version} ${qemu_sha}" > "latest"
}

create_qemu_cache_asset() {
	local component_name="$1"
	local component_version="$2"
	local qemu_cc_tarball_path=$(sudo find / -iname "${qemu_cc_tarball_name}")
	info "qemu cc tarball_path ${qemu_cc_tarball_path}"
	cp -a "${qemu_cc_tarball_path}" .
	sudo chown -R "${USER}:${USER}" .
	sha256sum "${component_name}" > "sha256sum-${component_name}"
	cat "sha256sum-${component_name}"
}

main() {
	mkdir -p "${WORKSPACE}/artifacts"
	pushd "${WORKSPACE}/artifacts"
	echo "Artifacts:"
	cache_qemu_artifacts
	ls -la "${WORKSPACE}/artifacts/"
	popd
	sync
}

main "$@"
