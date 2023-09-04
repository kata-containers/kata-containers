#!/bin/bash
# Copyright (c) 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "${script_dir}/../../scripts/lib.sh"

cache_target=""
ARCH=$(uname -m)
BRANCH=${BRANCH:-"main"}
REGISTRY=${REGISTRY:=ghcr.io/kata-containers/cached-components}

KERNEL_FLAVOUR="${KERNEL_FLAVOUR:-kernel}" # kernel | kernel-nvidia-gpu | kernel-experimental | kernel-arm-experimental | kernel-dragonball-experimental | kernel-tdx-experimental | kernel-nvidia-gpu-tdx-experimental | kernel-nvidia-gpu-snp
OVMF_FLAVOUR="${OVMF_FLAVOUR:-x86_64}" # x86_64 | tdx | sev
QEMU_FLAVOUR="${QEMU_FLAVOUR:-qemu}" # qemu | qemu-tdx-experimental | qemu-snp-experimental
ROOTFS_FLAVOUR="${ROOTFS_FLAVOUR:-image}" # image | image-tdx | initrd | initrd-sev | initrd-mariner

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
	# Changes to tools/packaging/kernel are covered by the kata_config_version check
	local kernel_last_commit="$(get_last_modification ${repo_root_dir}/tools/packaging/static-build/kernel)"
	local kernel_modules_tarball_path="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build/kata-static-kernel-sev-modules.tar.xz"

	# The ${vendor}-gpu kernels are based on an already existing entry, and does not require
	# adding a new entry to the versions.yaml.
	#
	# With this in mind, let's just make sure we get the version from correct entry in the
	# versions.yaml file.
	case ${KERNEL_FLAVOUR} in
		*"nvidia-gpu"*)
			KERNEL_FLAVOUR=${KERNEL_FLAVOUR//"-nvidia-gpu"/}
			;;
		*)
			;;
	esac

	case ${KERNEL_FLAVOUR} in
		"kernel-sev"|"kernel-snp")
			# In these cases, like "kernel-foo", it must be set to "kernel.foo" when looking at
			# the versions.yaml file
			current_kernel_version="$(get_from_kata_deps "assets.${KERNEL_FLAVOUR/-/.}.version")"
			;;
		*)
			current_kernel_version="$(get_from_kata_deps "assets.${KERNEL_FLAVOUR}.version")"
			;;
	esac

	local current_component_version="${current_kernel_version}-${current_kernel_kata_config_version}-${kernel_last_commit}"
	create_cache_asset "${kernel_tarball_name}" "${current_component_version}" "${current_kernel_image}"
	if [[ "${KERNEL_FLAVOUR}" == "kernel-sev" ]]; then
		module_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build/kernel-sev/builddir/kata-linux-${current_kernel_version#v}-${current_kernel_kata_config_version}/lib/modules/${current_kernel_version#v}"
		if [ ! -f "${kernel_modules_tarball_path}" ]; then
			tar cvfJ "${kernel_modules_tarball_path}" "${module_dir}/kernel/drivers/virt/coco/efi_secret/"
		fi
		create_cache_asset "kata-static-kernel-sev-modules.tar.xz" "${current_component_version}" "${current_kernel_image}"
	fi
}

cache_nydus_artifacts() {
	local nydus_tarball_name="kata-static-nydus.tar.xz"
	local current_nydus_version="$(get_from_kata_deps "externals.nydus.version")"
	create_cache_asset "${nydus_tarball_name}" "${current_nydus_version}" ""
}

cache_ovmf_artifacts() {
	local current_ovmf_version="$(get_from_kata_deps "externals.ovmf.${OVMF_FLAVOUR}.version")"
	case ${OVMF_FLAVOUR} in
		"tdx")
			ovmf_tarball_name="kata-static-tdvf.tar.xz"
			;;
		"x86_64")
			ovmf_tarball_name="kata-static-ovmf.tar.xz"
			;;
		"sev")
			ovmf_tarball_name="kata-static-ovmf-${OVMF_FLAVOUR}.tar.xz"
			;;
		*)
			die "invalid OVMF_FLAVOUR"
			;;
	esac
			
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
	local rootfs_tarball_name="kata-static-rootfs-${ROOTFS_FLAVOUR}.tar.xz"
	local current_rootfs_version="${osbuilder_last_commit}-${guest_image_last_commit}-${agent_last_commit}-${libs_last_commit}-${gperf_version}-${libseccomp_version}-${rust_version}-${ROOTFS_FLAVOUR}"
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

	local build_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build"
	local cache_dir="${script_dir}/cache"

	sudo chown -R "${USER}:${USER}" "${build_dir}/${component_name}"

	mkdir -p ${cache_dir}/${cache_target}
	cp "${build_dir}/${component_name}" "${cache_dir}/${cache_target}/${component_name}"
	sha256sum "${build_dir}/${component_name}" > "${cache_dir}/${cache_target}/sha256sum"
	cat "${cache_dir}/${cache_target}/sha256sum"
	echo "${component_version}" > "${cache_dir}/${cache_target}/latest"
	cat "${cache_dir}/${cache_target}/latest"
	echo "${component_image}" > "${cache_dir}/${cache_target}/latest_image"
	cat "${cache_dir}/${cache_target}/latest_image"

	docker build \
		-t ${REGISTRY}:${cache_target}-${BRANCH}-${ARCH} \
		-t ${REGISTRY}:${cache_target}-$(git rev-parse --short=12 HEAD)-${ARCH} \
		--build-arg COMPONENT=${cache_target} \
		${cache_dir}

	docker push ${REGISTRY}:${cache_target}-${BRANCH}-${ARCH}
	docker push ${REGISTRY}:${cache_target}-$(git rev-parse HEAD)-${ARCH}

	rm -rf "${cache_dir}/${cache_target}"
}

help() {
echo "$(cat << EOF
Usage: $0 <options>

Args:

options:
-h|--help       : Show this help
--cache=<asset> :
	cloud-hypervisor
	cloud-hypervisor-glibc
	firecracker
	kernel
	kernel-dragonball-experimental
	kernel-nvidia-gpu
	kernel-nvidia-gpu-snp
	kernel-nvidia-gpu-tdx-experimental
	kernel-sev
	kernel-tdx-experimentall
	nydus
	ovmf
	ovmf-sev
	qemu
	qemu-snp-experimental
	qemu-tdx-experimental
	rootfs-image
	rootfs-image-tdx
	rootfs-initrd
	rootfs-initrd-mariner
	rootfs-initrd-sev
	shim-v2
	tdvf
	virtiofsd
EOF
)"
}

main() {
	local OPTIND
	while getopts "hs-:" opt; do
		case $opt in
		-)
			case "${OPTARG}" in
			cache=*)
				cache_target=(${OPTARG#*=cache-})
				;;
			help)
				help
				;;
			*)
				help
				;;
			esac
			;;
		h) help ;;
		*) help ;;
		esac
	done
	shift $((OPTIND - 1))

	case ${cache_target} in
		cloud-hypervisor | cloud-hypervisor-glibc)
			cache_clh_artifacts
			;;
		firecracker)
			cache_firecracker_artifacts
			;;
		kernel | kernel-dragonball-experimental | kernel-nvidia-gpu | kernel-nvidia-gpu-snp | kernel-nvidia-gpu-tdx-experimental | kernel-sev | kernel-tdx-experimental)
			export KERNEL_FLAVOUR=${cache_target}
			cache_kernel_artifacts
			;;
		nydus)
			cache_nydus_artifacts
			;;
		ovmf)
			export OVMF_FLAVOUR=x86_64
			cache_ovmf_artifacts
			;;
		ovmf-sev)
			export OVMF_FLAVOUR=sev
			cache_ovmf_artifacts
			;;
		qemu | qemu-snp-experimental | qemu-tdx-experimental)
			export QEMU_FLAVOUR=${cache_target}
			cache_qemu_artifacts
			;;
		rootfs-image | rootfs-image-tdx | rootfs-initrd | rootfs-initrd-mariner | rootfs-initrd-sev)
			export ROOTFS_FLAVOUR=${cache_target#"rootfs-"}
			cache_rootfs_artifacts
			;;
		shim-v2)
			cache_shim_v2_artifacts
			;;
		tdvf)
			export OVMF_FLAVOUR=tdx
			cache_ovmf_artifacts
			;;
		virtiofsd)
			cache_virtiofsd_artifacts
			;;
		*)
			die "invalid cache target ${cache_target}"
			;;
	esac
}

main "$@"
