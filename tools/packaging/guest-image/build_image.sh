#!/usr/bin/env bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

[ -z "${DEBUG}" ] || set -x

set -o errexit
set -o nounset
set -o pipefail

readonly script_name="$(basename "${BASH_SOURCE[0]}")"
readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly packaging_root_dir="$(cd "${script_dir}/../" && pwd)"

source "${packaging_root_dir}/scripts/lib.sh"

readonly osbuilder_dir="$(cd "${repo_root_dir}/tools/osbuilder" && pwd)"

export GOPATH=${GOPATH:-${HOME}/go}

final_image_name="kata-containers"
final_initrd_name="kata-containers-initrd"
image_initrd_extension=".img"

arch_target="$(uname -m)"
final_initrd_name="kata-containers-initrd"
image_initrd_extension=".img"

build_initrd() {
	info "Build initrd"
	info "initrd os: $initrd_distro"
	info "initrd os version: $initrd_os_version"
	local rootfs_build_dest="${builddir}/initrd-image"
	export DISTRO="$initrd_distro"
	export OS_VERSION="${initrd_os_version}"
	export USE_DOCKER=1
	export AGENT_INIT="yes"
	# ROOTFS_BUILD_DEST is a Make variable
	# SNP will also use the SEV guest module
	if [[ "${AA_KBC:-}" == "offline_sev_kbc" || "${AA_KBC:-}" == "online_sev_kbc" ]]; then
		config_version=$(get_config_version)
		kernel_version="$(get_from_kata_deps "assets.kernel.sev.version")"
		kernel_version=${kernel_version#v}
		module_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build/cc-sev-kernel/builddir/kata-linux-${kernel_version}-${config_version}/lib/modules/${kernel_version}"
		sudo -E PATH="$PATH" make rootfs ROOTFS_BUILD_DEST="${rootfs_build_dest}" KERNEL_MODULES_DIR="${module_dir}"
	else
		sudo -E PATH="$PATH" make rootfs ROOTFS_BUILD_DEST="${rootfs_build_dest}"
	fi

	if [ -n "${INCLUDE_ROOTFS:-}" ]; then
		sudo cp -RL --preserve=mode "${INCLUDE_ROOTFS}/." "${rootfs_build_dest}/${initrd_distro}_rootfs/"
	fi
	sudo -E PATH="$PATH" make initrd ROOTFS_BUILD_DEST="${rootfs_build_dest}"
	mv "kata-containers-initrd.img" "${install_dir}/${initrd_name}"
	(
		cd "${install_dir}"
		ln -sf "${initrd_name}" "${final_initrd_name}${image_initrd_extension}"
	)
}

build_image() {
	info "Build image"
	info "image os: $img_distro"
	info "image os version: $img_os_version"
	sudo -E PATH="${PATH}" make image \
		DISTRO="${img_distro}" \
		DEBUG="${DEBUG:-}" \
		USE_DOCKER="1" \
		IMG_OS_VERSION="${img_os_version}" \
		ROOTFS_BUILD_DEST="${builddir}/rootfs-image"
	mv -f "kata-containers.img" "${install_dir}/${image_name}"
	if [ -e "root_hash.txt" ]; then
		[ -z "${root_hash_suffix}" ] && root_hash_suffix=vanilla
		mv "${repo_root_dir}/tools/osbuilder/root_hash.txt" "${repo_root_dir}/tools/osbuilder/root_hash_${root_hash_suffix}.txt"
	fi
	(
		cd "${install_dir}"
		ln -sf "${image_name}" "${final_image_name}${image_initrd_extension}"
	)
}

usage() {
	return_code=${1:-0}
	cat <<EOF
Create image and initrd in a tarball for kata containers.
Use it to build an image to distribute kata.

Usage:
${script_name} [options]

Options:
 --imagetype=${image_type}
 --prefix=${prefix}
 --destdir=${destdir}
 --image_initrd_suffix=${image_initrd_suffix}
EOF

	exit "${return_code}"
}

main() {
	image_type=image
	destdir="$PWD"
	prefix="/opt/kata"
	image_initrd_suffix=""
	root_hash_suffix=""
	builddir="${PWD}"
	while getopts "h-:" opt; do
		case "$opt" in
		-)
			case "${OPTARG}" in
			imagetype=image)
				image_type=image
				#image information
				img_distro=$(get_from_kata_deps "assets.image.architecture.${arch_target}.name")
				img_os_version=$(get_from_kata_deps "assets.image.architecture.${arch_target}.version")
				image_name="kata-${img_distro}-${img_os_version}.${image_type}"
				;;
			imagetype=initrd)
				image_type=initrd
				#initrd information
				initrd_distro=$(get_from_kata_deps "assets.initrd.architecture.${arch_target}.name")
				initrd_os_version=$(get_from_kata_deps "assets.initrd.architecture.${arch_target}.version")
				initrd_name="kata-${initrd_distro}-${initrd_os_version}.${image_type}"
				;;
			image_initrd_suffix=*)
				image_initrd_suffix=${OPTARG#*=}
				if [ "${image_initrd_suffix}" == "sev" ]; then
					initrd_distro=$(get_from_kata_deps "assets.initrd.architecture.${arch_target}.sev.name")
					initrd_os_version=$(get_from_kata_deps "assets.initrd.architecture.${arch_target}.sev.version")
					initrd_name="kata-${initrd_distro}-${initrd_os_version}-${image_initrd_suffix}.${image_type}"
					final_initrd_name="${final_initrd_name}-${image_initrd_suffix}"
				elif [ -n "${image_initrd_suffix}" ]; then
					img_distro=$(get_from_kata_deps "assets.image.architecture.${arch_target}.name")
					img_os_version=$(get_from_kata_deps "assets.image.architecture.${arch_target}.version")
					image_name="kata-${img_distro}-${img_os_version}-${image_initrd_suffix}.${image_type}"
					final_image_name="${final_image_name}-${image_initrd_suffix}"

					initrd_distro=$(get_from_kata_deps "assets.initrd.architecture.${arch_target}.name")
					initrd_os_version=$(get_from_kata_deps "assets.initrd.architecture.${arch_target}.version")
					initrd_name="kata-${initrd_distro}-${initrd_os_version}-${image_initrd_suffix}.${image_type}"
					final_initrd_name="${final_initrd_name}-${image_initrd_suffix}"
				fi
				;;
			root_hash_suffix=*)
				root_hash_suffix=${OPTARG#*=}
				;;
			prefix=*)
				prefix=${OPTARG#*=}
				;;
			destdir=*)
				destdir=${OPTARG#*=}
				;;
			builddir=*)
				builddir=${OPTARG#*=}
				;;
			*)
				echo >&2 "ERROR: Invalid option -$opt${OPTARG}"
				usage 1
				;;
			esac
			;;
		h) usage 0 ;;
		*)
			echo "Invalid option $opt"
			usage 1
			;;
		esac
	done
	readonly destdir
	readonly builddir

	echo "build ${image_type}"



	install_dir="${destdir}/${prefix}/share/kata-containers/"
	readonly install_dir

	mkdir -p "${install_dir}"

	pushd "${osbuilder_dir}"
	case "${image_type}" in
	initrd) build_initrd ;;
	image) build_image ;;
	esac

	popd
}

main $*