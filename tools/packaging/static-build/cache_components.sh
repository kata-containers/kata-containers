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
        [ "${TEE}" == "snp" ] && current_qemu_version=$(get_from_kata_deps "assets.hypervisor.qemu.snp.tag")
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
	local current_kernel_kata_config_version="$(cat ${repo_root_dir}/tools/packaging/kernel/kata_config_version)"
	local kernel_modules_tarball_path="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build/kata-static-cc-sev-kernel-modules.tar.xz"
	if [ -n "${TEE}" ]; then
		kernel_tarball_name="kata-static-cc-${TEE}-kernel.tar.xz"
		[ "${TEE}" == "tdx" ] && current_kernel_version="$(get_from_kata_deps "assets.kernel.${TEE}.tag")"
		[ "${TEE}" == "sev" ] && current_kernel_version="$(get_from_kata_deps "assets.kernel.${TEE}.version")"
	fi
	create_cache_asset "${kernel_tarball_name}" "${current_kernel_version}-${current_kernel_kata_config_version}" "${current_kernel_image}"

	if [ "${TEE}" == "sev" ]; then
		module_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build/cc-sev-kernel/builddir/kata-linux-${current_kernel_version#v}-$(get_config_version)/lib/modules/${current_kernel_version#v}"
		if [ ! -f "${kernel_modules_tarball_path}" ]; then
			tar cvfJ "${kernel_modules_tarball_path}" "${module_dir}/kernel/drivers/virt/coco/efi_secret/"
		fi
		create_cache_asset "kata-static-cc-sev-kernel-modules.tar.xz" "${current_kernel_version}-${current_kernel_kata_config_version}" "${current_kernel_image}"
	fi

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
		"ovmf")
			firmware_tarball_name="kata-static-cc-sev-ovmf.tar.xz"
			current_firmware_image="$(get_ovmf_image_name)"
			current_firmware_version="$(get_from_kata_deps "externals.ovmf.sev.version")"
			;;
		*)
			die "Not a valid firmware (td-shim, tdvf, ovmf) wass set as the FIRMWARE environment variable."

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

cache_rootfs_artifacts() {
	# We need to remove `-dirty` from teh osbuilder_last_commit as the rootfs artefacts are generated on that folder
	local osbuilder_last_commit="$(echo $(get_last_modification "${repo_root_dir}/tools/osbuilder") | sed s/-dirty//)"
	local guest_image_last_commit="$(get_last_modification "${repo_root_dir}/tools/packaging/guest-image")"
	local agent_last_commit="$(get_last_modification "${repo_root_dir}/src/agent")"
	local libs_last_commit="$(get_last_modification "${repo_root_dir}/src/libs")"
	local attestation_agent_version="$(get_from_kata_deps "externals.attestation-agent.version")"
	local gperf_version="$(get_from_kata_deps "externals.gperf.version")"
	local libseccomp_version="$(get_from_kata_deps "externals.libseccomp.version")"
	local pause_version="$(get_from_kata_deps "externals.pause.version")"
	local rust_version="$(get_from_kata_deps "languages.rust.meta.newest-version")"
	local rootfs_tarball_name="kata-static-cc-rootfs-image.tar.xz"
	local aa_kbc="offline_fs_kbc"
	local initramfs_last_commit=""
	local image_type="image"
	local root_hash_vanilla="${repo_root_dir}/tools/osbuilder/root_hash_vanilla.txt"
	local root_hash_tdx=""
	if [ -n "${TEE}" ]; then
		if [ "${TEE}" == "tdx" ]; then
			rootfs_tarball_name="kata-static-rootfs-image-tdx.tar.xz"
			aa_kbc="cc_kbc_tdx"
			image_type="image"
			root_hash_vanilla=""
			root_hash_tdx="${repo_root_dir}/tools/osbuilder/root_hash_tdx.txt"
		fi
		if [ "${TEE}" == "sev" ]; then
			root_hash_vanilla=""
			rootfs_tarball_name="kata-static-rootfs-initrd-sev.tar.xz"
			aa_kbc="online_sev_kbc"
			image_type="initrd"
			initramfs_last_commit="$(get_initramfs_image_name)"
		fi
	fi
	local current_rootfs_version="${osbuilder_last_commit}-${guest_image_last_commit}-${initramfs_last_commit}-${agent_last_commit}-${libs_last_commit}-${attestation_agent_version}-${gperf_version}-${libseccomp_version}-${pause_version}-${rust_version}-${image_type}-${aa_kbc}"
	create_cache_asset "${rootfs_tarball_name}" "${current_rootfs_version}" "" "${root_hash_vanilla}" "${root_hash_tdx}"
}

cache_shim_v2_artifacts() {
	local shim_v2_tarball_name="kata-static-cc-shim-v2.tar.xz"
	local shim_v2_last_commit="$(get_last_modification "${repo_root_dir}/src/runtime")"
	local protocols_last_commit="$(get_last_modification "${repo_root_dir}/src/libs/protocols")"
	local runtime_rs_last_commit="$(get_last_modification "${repo_root_dir}/src/runtime-rs")"
	local golang_version="$(get_from_kata_deps "languages.golang.meta.newest-version")"
	local rust_version="$(get_from_kata_deps "languages.rust.meta.newest-version")"
	local current_shim_v2_version="${shim_v2_last_commit}-${protocols_last_commit}-${runtime_rs_last_commit}-${golang_version}-${rust_version}"
	local current_shim_v2_image="$(get_shim_v2_image_name)"
	create_cache_asset "${shim_v2_tarball_name}" "${current_shim_v2_version}" "${current_shim_v2_image}" "${repo_root_dir}/tools/osbuilder/root_hash_vanilla.txt" "${repo_root_dir}/tools/osbuilder/root_hash_tdx.txt"
}

create_cache_asset() {
	local component_name="${1}"
	local component_version="${2}"
	local component_image="${3}"
	local root_hash_vanilla="${4:-""}"
	local root_hash_tdx="${5:-""}"

	sudo cp "${repo_root_dir}/tools/packaging/kata-deploy/local-build/build/${component_name}" .
	sudo chown -R "${USER}:${USER}" .
	sha256sum "${component_name}" > "sha256sum-${component_name}"
	cat "sha256sum-${component_name}"
	echo "${component_version}" > "latest"
	cat "latest"
	echo "${component_image}" > "latest_image"
	cat "latest_image"
	if [ -n "${root_hash_vanilla}" ]; then
		local cached_root_hash_vanilla="$(basename ${root_hash_vanilla})"
		sudo cp "${root_hash_vanilla}" "${cached_root_hash_vanilla}"
		sudo chown -R "${USER}:${USER}" "${cached_root_hash_vanilla}"
		echo "${cached_root_hash_vanilla}: $(cat "${cached_root_hash_vanilla}")"
	fi
	if [ -n "${root_hash_tdx}" ]; then
		local cached_root_hash_tdx="$(basename ${root_hash_tdx})"
		sudo cp "${root_hash_tdx}" "${cached_root_hash_tdx}"
		sudo chown -R "${USER}:${USER}" "${cached_root_hash_tdx}"
		echo "${cached_root_hash_tdx}: $(cat "${cached_root_hash_tdx}")"
	fi
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
			  * ovmf
		-s	Shim v2 cache
		-v	Virtiofsd cache
		-r	Rootfs Cache
			* can receive a TEE environment variable value, valid values are:
			  * tdx
			  If not TEE environment is passed, the Rootfs Image will be built without TEE support.
		-h	Shows help
EOF
)"
}

main() {
	local cloud_hypervisor_component="${cloud_hypervisor_component:-}"
	local qemu_component="${qemu_component:-}"
	local kernel_component="${kernel_component:-}"
	local firmware_component="${firmware_component:-}"
	local shim_v2_component="${shim_v2_component:-}"
	local virtiofsd_component="${virtiofsd_component:-}"
	local rootfs_component="${rootfs_component:-}"
	local OPTIND
	while getopts ":ckqfvrsh:" opt
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
		s)
			shim_v2_component="1"
			;;
		v)
			virtiofsd_component="1"
			;;
		r)
			rootfs_component="1"
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
	[[ -z "${shim_v2_component}" ]] && \
	[[ -z "${virtiofsd_component}" ]] && \
	[[ -z "${rootfs_component}" ]] && \
		help && die "Must choose at least one option"

	mkdir -p "${WORKSPACE}/artifacts"
	pushd "${WORKSPACE}/artifacts"
	echo "Artifacts:"

	[ "${cloud_hypervisor_component}" == "1" ] && cache_clh_artifacts
	[ "${kernel_component}" == "1" ] && cache_kernel_artifacts
	[ "${qemu_component}" == "1" ] && cache_qemu_artifacts
	[ "${firmware_component}" == "1" ] && cache_firmware_artifacts
	[ "${shim_v2_component}" == "1" ] && cache_shim_v2_artifacts
	[ "${virtiofsd_component}" == "1" ] && cache_virtiofsd_artifacts
	[ "${rootfs_component}" == "1" ] && cache_rootfs_artifacts

	ls -la "${WORKSPACE}/artifacts/"
	popd
	sync
}

main "$@"
