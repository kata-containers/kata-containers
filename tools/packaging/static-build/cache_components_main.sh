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

KERNEL_FLAVOUR="${KERNEL_FLAVOUR:-kernel}" # kernel | kernel-experimental | kernel-arm-experimental | kernel-dragonball-experimental | kernel-tdx-experimental
OVMF_FLAVOUR="${OVMF_FLAVOUR:-x86_64}" # x86_64 | tdx
QEMU_FLAVOUR="${QEMU_FLAVOUR:-qemu}" # qemu | qemu-tdx-experimental
ROOTFS_IMAGE_TYPE="${ROOTFS_IMAGE_TYPE:-image}" # image | initrd

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

cache_nydus_artifacts() {
	local nydus_tarball_name="kata-static-nydus.tar.xz"
	local current_nydus_version="$(get_from_kata_deps "externals.nydus.version")"
	create_cache_asset "${nydus_tarball_name}" "${current_nydus_version}" ""
}

cache_ovmf_artifacts() {
	local current_ovmf_version="$(get_from_kata_deps "externals.ovmf.${OVMF_FLAVOUR}.version")"
	[ "${OVMF_FLAVOUR}" == "tdx" ] && OVMF_FLAVOUR="tdvf"
	local ovmf_tarball_name="kata-static-${OVMF_FLAVOUR}.tar.xz"
	local current_ovmf_image="$(get_ovmf_image_name)"
	create_cache_asset "${ovmf_tarball_name}" "${current_ovmf_version}" "${current_ovmf_image}"
}

cache_qemu_artifacts() {
	local qemu_tarball_name="kata-static-${QEMU_FLAVOUR}.tar.xz"
	local current_qemu_version=$(get_from_kata_deps "assets.hypervisor.${QEMU_FLAVOUR}.version")
	[ -z "${current_qemu_version}" ] && current_qemu_version=$(get_from_kata_deps "assets.hypervisor.${QEMU_FLAVOUR}.tag")
	local qemu_sha=$(calc_qemu_files_sha256sum)
	local current_qemu_image="$(get_qemu_image_name)"
	create_cache_asset "${qemu_tarball_name}" "${current_qemu_version}-${qemu_sha}" "${current_qemu_image}"
}

cache_rootfs_artifacts() {
	local osbuilder_last_commit="$(get_last_modification "${repo_root_dir}/tools/osbuilder")"
	local guest_image_last_commit="$(get_last_modification "${repo_root_dir}/tools/packaging/guest-image")"
	local agent_last_commit="$(get_last_modification "${repo_root_dir}/src/agent")"
	local libs_last_commit="$(get_last_modification "${repo_root_dir}/src/libs")"
	local gperf_version="$(get_from_kata_deps "externals.gperf.version")"
	local libseccomp_version="$(get_from_kata_deps "externals.libseccomp.version")"
	local rust_version="$(get_from_kata_deps "languages.rust.meta.newest-version")"
	local rootfs_tarball_name="kata-static-rootfs-${ROOTFS_IMAGE_TYPE}.tar.xz"
	local current_rootfs_version="${osbuilder_last_commit}-${guest_image_last_commit}-${agent_last_commit}-${libs_last_commit}-${gperf_version}-${libseccomp_version}-${rust_version}-${ROOTFS_IMAGE_TYPE}"
	create_cache_asset "${rootfs_tarball_name}" "${current_rootfs_version}" ""
}

cache_shim_v2_artifacts() {
	local shim_v2_tarball_name="kata-static-shim-v2.tar.xz"
	local shim_v2_last_commit="$(get_last_modification "${repo_root_dir}/src/runtime")"
	local protocols_last_commit="$(get_last_modification "${repo_root_dir}/src/libs/protocols")"
	local runtime_rs_last_commit="$(get_last_modification "${repo_root_dir}/src/runtime-rs")"
	local golang_version="$(get_from_kata_deps "languages.golang.meta.newest-version")"
	local rust_version="$(get_from_kata_deps "languages.rust.meta.newest-version")"
	local current_shim_v2_version="${shim_v2_last_commit}-${protocols_last_commit}-${runtime_rs_last_commit}-${golang_version}-${rust_version}"
	local current_shim_v2_image="$(get_shim_v2_image_name)"
	create_cache_asset "${shim_v2_tarball_name}" "${current_shim_v2_version}" "${current_shim_v2_image}"
}

cache_virtiofsd_artifacts() {
	local virtiofsd_tarball_name="kata-static-virtiofsd.tar.xz"
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
		-F	Firecracker cache
		-k	Kernel cache
			* Export KERNEL_FLAVOUR="kernel | kernel-experimental | kernel-arm-experimental | kernel-dragonball-experimental | kernel-tdx-experimental" for a specific build
			  The default KERNEL_FLAVOUR value is "kernel"
		-n	Nydus cache
		-q 	QEMU cache
			* Export QEMU_FLAVOUR="qemu | qemu-tdx-experimental" for a specific build
			  The default QEMU_FLAVOUR value is "qemu"
		-r 	RootFS cache
			* Export ROOTFS_IMAGE_TYPE="image|initrd" for one of those two types
			  The default ROOTFS_IMAGE_TYPE value is "image"
		-s	Shim v2 cache
		-v	VirtioFS cache
		-h	Shows help
EOF
)"
}

main() {
	local cloud_hypervisor_component="${cloud_hypervisor_component:-}"
	local firecracker_component="${firecracker_component:-}"
	local kernel_component="${kernel_component:-}"
	local nydus_component="${nydus_component:-}"
	local ovmf_component="${ovmf_component:-}"
	local qemu_component="${qemu_component:-}"
	local rootfs_component="${rootfs_component:-}"
	local shim_v2_component="${shim_v2_component:-}"
	local virtiofsd_component="${virtiofsd_component:-}"
	local OPTIND
	while getopts ":cFknoqrsvh:" opt
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
		n)
			nydus_component="1"
			;;
		o)
			ovmf_component="1"
			;;
		q)
			qemu_component="1"
			;;
		r)
			rootfs_component="1"
			;;
		s)
			shim_v2_component="1"
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
	[[ -z "${firecracker_component}" ]] && \
	[[ -z "${kernel_component}" ]] && \
	[[ -z "${nydus_component}" ]] && \
	[[ -z "${ovmf_component}" ]] && \
	[[ -z "${qemu_component}" ]] && \
	[[ -z "${rootfs_component}" ]] && \
	[[ -z "${shim_v2_component}" ]] && \
	[[ -z "${virtiofsd_component}" ]] && \
		help && die "Must choose at least one option"

	mkdir -p "${WORKSPACE}/artifacts"
	pushd "${WORKSPACE}/artifacts"
	echo "Artifacts:"

	[ "${cloud_hypervisor_component}" == "1" ] && cache_clh_artifacts
	[ "${firecracker_component}" == "1" ] && cache_firecracker_artifacts
	[ "${kernel_component}" == "1" ] && cache_kernel_artifacts
	[ "${nydus_component}" == "1" ] && cache_nydus_artifacts
	[ "${ovmf_component}" == "1" ] && cache_ovmf_artifacts
	[ "${qemu_component}" == "1" ] && cache_qemu_artifacts
	[ "${rootfs_component}" == "1" ] && cache_rootfs_artifacts
	[ "${shim_v2_component}" == "1" ] && cache_shim_v2_artifacts
	[ "${virtiofsd_component}" == "1" ] && cache_virtiofsd_artifacts

	ls -la "${WORKSPACE}/artifacts/"
	popd
	sync
}

main "$@"
