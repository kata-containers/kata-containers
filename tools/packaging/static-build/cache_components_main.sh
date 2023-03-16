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

KERNEL_FLAVOUR="${KERNEL_FLAVOUR:-kernel}" # kernel | kernel-experimental | kernel-arm-experimetnal | kernel-dragonball-experimental

cache_clh_artifacts() {
	local clh_tarball_name="kata-static-cloud-hypervisor.tar.xz"
	local current_clh_version="$(get_from_kata_deps "assets.hypervisor.cloud_hypervisor.version")"
	create_cache_asset "${clh_tarball_name}" "${current_clh_version}" ""
}

cache_firecracker_artifacts() {
	local fc_tarball_name="kata-static-firecracker.tar.xz"
	local current_fc_version="$(get_from_kata_deps "assets.hypervisor.firecracker.version")"
	create_cache_asset "${fc_tarball_name}" "${current_fc_version}" ""
}

cache_kernel_artifacts() {
	local kernel_tarball_name="kata-static-${KERNEL_FLAVOUR}.tar.xz"
	local current_kernel_image="$(get_kernel_image_name)"
	local current_kernel_kata_config_version="$(cat ${repo_root_dir}/tools/packaging/kernel/kata_config_version)"
	local current_kernel_version="$(get_from_kata_deps "assets.${KERNEL_FLAVOUR}.version")-${current_kernel_kata_config_version}"
	create_cache_asset "${kernel_tarball_name}" "${current_kernel_version}" "${current_kernel_image}"
}

create_cache_asset() {
	local component_name="${1}"
	local component_version="${2}"
	local component_image="${3}"

	sudo cp "${repo_root_dir}/tools/packaging/kata-deploy/local-build/build/${component_name}" .
	sudo chown -R "${USER}:${USER}" .
	sha256sum "${component_name}" > "sha256sum-${component_name}"
	cat "sha256sum-${component_name}"
	echo "${component_version}" > "latest"
	cat "latest"
	echo "${component_image}" > "latest_image"
	cat "latest_image"
}

help() {
echo "$(cat << EOF
Usage: $0 "[options]"
	Description:
	Builds the cache of several kata components.
	Options:
		-c	Cloud hypervisor cache
		-F	Firecracker cache
		-k	Kernel cache
			* Export KERNEL_FLAVOUR="kernel|kernek-experimental|kernel-arm-experimental|kernel-dragonball-experimental" for a specific build
			  The default KERNEL_FLAVOUR value is "kernel"
		-h	Shows help
EOF
)"
}

main() {
	local cloud_hypervisor_component="${cloud_hypervisor_component:-}"
	local firecracker_component="${firecracker_component:-}"
	local kernel_component="${kernel_component:-}"
	local OPTIND
	while getopts ":cFkh:" opt
	do
		case "$opt" in
		c)
			cloud_hypervisor_component="1"
			;;
		F)
			firecracker_component="1"
			;;
		k)
			kernel_component="1"
			;;
		h)
			help
			exit 0;
			;;
		:)
			echo "Missing argument for -$OPTARG";
			help
			exit 1;
			;;
		esac
	done
	shift $((OPTIND-1))

	[[ -z "${cloud_hypervisor_component}" ]] && \
	[[ -z "${firecracker_component}" ]] && \
	[[ -z "${kernel_component}" ]] && \
		help && die "Must choose at least one option"

	mkdir -p "${WORKSPACE}/artifacts"
	pushd "${WORKSPACE}/artifacts"
	echo "Artifacts:"

	[ "${cloud_hypervisor_component}" == "1" ] && cache_clh_artifacts
	[ "${firecracker_component}" == "1" ] && cache_firecracker_artifacts
	[ "${kernel_component}" == "1" ] && cache_kernel_artifacts

	ls -la "${WORKSPACE}/artifacts/"
	popd
	sync
}

main "$@"
