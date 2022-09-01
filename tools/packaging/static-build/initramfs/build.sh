#!/usr/bin/env bash
#
# Copyright (c) 2022 Intel
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root_dir="$(cd "${script_dir}/../../../.." && pwd)"
readonly initramfs_builder="${script_dir}/build-initramfs.sh"
readonly default_install_dir="$(cd "${script_dir}/../../kernel" && pwd)"

source "${script_dir}/../../scripts/lib.sh"

container_image="kata-initramfs-builder"
kata_version="${kata_version:-}"
cryptsetup_repo="${cryptsetup_repo:-}"
cryptsetup_version="${cryptsetup_version:-}"
lvm2_repo="${lvm2_repo:-}"
lvm2_version="${lvm2_version:-}"
package_output_dir="${package_output_dir:-}"

[ -n "${cryptsetup_repo}" ] || cryptsetup_repo=$(get_from_kata_deps "externals.cryptsetup.url" "${kata_version}")
[ -n "${cryptsetup_version}" ] || cryptsetup_version=$(get_from_kata_deps "externals.cryptsetup.version" "${kata_version}")
[ -n "${lvm2_repo}" ] || lvm2_repo=$(get_from_kata_deps "externals.lvm2.url" "${kata_version}")
[ -n "${lvm2_version}" ] || lvm2_version=$(get_from_kata_deps "externals.lvm2.version" "${kata_version}")

[ -n "${cryptsetup_repo}" ] || die "Failed to get cryptsetup repo"
[ -n "${cryptsetup_version}" ] || die "Failed to get cryptsetup version"
[ -n "${lvm2_repo}" ] || die "Failed to get lvm2 repo"
[ -n "${lvm2_version}" ] || die "Failed to get lvm2 version"

sudo docker build \
	-t "${container_image}" "${script_dir}"

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--env cryptsetup_repo="${cryptsetup_repo}" \
	--env cryptsetup_version="${cryptsetup_version}" \
	--env lvm2_repo="${lvm2_repo}" \
	--env lvm2_version="${lvm2_version}" \
	"${container_image}" \
	bash -c "${initramfs_builder} ${default_install_dir}"
