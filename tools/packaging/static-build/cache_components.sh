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

export KATA_BUILD_CC="${KATA_BUILD_CC:-}"
export TEE="${TEE:-}"
export FIRMWARE="${FIRMWARE:-}"

cache_qemu_artifacts() {
	local qemu_tarball_name="kata-static-cc-qemu.tar.xz"
	local current_qemu_version=$(get_from_kata_deps "assets.hypervisor.qemu.version")
	if [ -n "${TEE}" ]; then
		qemu_tarball_name="kata-static-cc-${TEE}-qemu.tar.xz"
		[ "${TEE}" == "tdx" ] && current_qemu_version=$(get_from_kata_deps "assets.hypervisor.qemu.tdx.tag")
	fi
	local qemu_sha=$(calc_qemu_files_sha256sum)
	local current_qemu_image="$(get_qemu_image_name)"

	create_cache_asset "${qemu_tarball_name}" "${current_qemu_version}-${qemu_sha}" "${current_qemu_image}"
}

cache_clh_artifacts() {
	local clh_tarball_name="kata-static-cc-cloud-hypervisor.tar.xz"
	local current_clh_version=$(get_from_kata_deps "assets.hypervisor.cloud_hypervisor.version")
	create_cache_asset "${clh_tarball_name}" "${current_clh_version}" ""
}

cache_kernel_artifacts() {
	local kernel_tarball_name="kata-static-cc-kernel.tar.xz"
	local current_kernel_image="$(get_kernel_image_name)"
	local current_kernel_version="$(get_from_kata_deps "assets.kernel.version")"
	if [ -n "${TEE}" ]; then
		kernel_tarball_name="kata-static-cc-${TEE}-kernel.tar.xz"
		[ "${TEE}" == "tdx" ] && current_kernel_version="$(get_from_kata_deps "assets.kernel.${TEE}.tag")"
		[ "${TEE}" == "sev" ] && current_kernel_version="$(get_from_kata_deps "assets.kernel.${TEE}.version")"
	fi
	create_cache_asset "${kernel_tarball_name}" "${current_kernel_version}" "${current_kernel_image}"
}

cache_firmware_artifacts() {
	case ${FIRMWARE} in
		"td-shim")
			firmware_tarball_name="kata-static-cc-tdx-td-shim.tar.xz"
			current_firmware_image="$(get_td_shim_image_name)"
			current_firmware_version="$(get_from_kata_deps "externals.td-shim.version")-$(get_from_kata_deps "externals.td-shim.toolchain")"
			;;
		"tdvf")
			firmware_tarball_name="kata-static-cc-tdx-tdvf.tar.xz"
			current_firmware_image="$(get_ovmf_image_name)"
			current_firmware_version="$(get_from_kata_deps "externals.ovmf.tdx.version")"
			;;
		*)
			die "Not a valid firmware (td-shim, tdvf) wass set as the FIRMWARE environment variable."

			;;
	esac
	create_cache_asset "${firmware_tarball_name}" "${current_firmware_version}" "${current_firmware_image}"
}

cache_virtiofsd_artifacts() {
	local virtiofsd_tarball_name="kata-static-cc-virtiofsd.tar.xz"
	local current_virtiofsd_version="$(get_from_kata_deps "externals.virtiofsd.version")-$(get_from_kata_deps "externals.virtiofsd.toolchain")"
	local current_virtiofsd_image="$(get_virtiofsd_image_name)"
	create_cache_asset "${virtiofsd_tarball_name}" "${current_virtiofsd_version}" "${current_virtiofsd_image}"
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
		-k	Kernel cache
			* Can receive a TEE environnment variable value, valid values are:
			  * tdx
			  If no TEE environment is passed, the kernel is built without TEE support.
		-q	Qemu cache
			* Can receive a TEE environnment variable value, valid values are:
			  * tdx
			  If no TEE environment is passed, QEMU is built without TEE support.
		-f	Firmware cache
			* Requires FIRMWARE environment variable set, valid values are:
			  * tdvf
			  * td-shim
		-v	Virtiofsd cache
		-h	Shows help
EOF
)"
}

main() {
	local cloud_hypervisor_component="${cloud_hypervisor_component:-}"
	local qemu_component="${qemu_component:-}"
	local kernel_component="${kernel_component:-}"
	local firmware_component="${firmware_component:-}"
	local virtiofsd_component="${virtiofsd_component:-}"
	local OPTIND
	while getopts ":ckqfvh:" opt
	do
		case "$opt" in
		c)
			cloud_hypervisor_component="1"
			;;
		k)
			kernel_component="1"
			;;
		q)
			qemu_component="1"
			;;
		f)
			firmware_component="1"
			;;
		v)
			virtiofsd_component="1"
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
	[[ -z "${kernel_component}" ]] && \
	[[ -z "${qemu_component}" ]] && \
	[[ -z "${firmware_component}" ]] && \
	[[ -z "${virtiofsd_component}" ]] && \
		help && die "Must choose at least one option"

	mkdir -p "${WORKSPACE}/artifacts"
	pushd "${WORKSPACE}/artifacts"
	echo "Artifacts:"

	[ "${cloud_hypervisor_component}" == "1" ] && cache_clh_artifacts
	[ "${kernel_component}" == "1" ] && cache_kernel_artifacts
	[ "${qemu_component}" == "1" ] && cache_qemu_artifacts
	[ "${firmware_component}" == "1" ] && cache_firmware_artifacts
	[ "${virtiofsd_component}" == "1" ] && cache_virtiofsd_artifacts

	ls -la "${WORKSPACE}/artifacts/"
	popd
	sync
}

main "$@"
