#!/bin/bash
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
readonly repo_root_dir="$(cd "${script_dir}/../../../" && pwd)"
readonly osbuilder_dir="$(cd "${repo_root_dir}/tools/osbuilder" && pwd)"

export GOPATH=${GOPATH:-${HOME}/go}
source "${packaging_root_dir}/scripts/lib.sh"

arch_target="$(uname -m)"

build_initrd() {
	info "Build initrd"
	info "initrd os: $initrd_distro"
	info "initrd os version: $initrd_os_version"
	sudo -E PATH="$PATH" make initrd \
		DISTRO="$initrd_distro" \
		DEBUG="${DEBUG:-}" \
		OS_VERSION="${initrd_os_version}" \
		ROOTFS_BUILD_DEST="${builddir}/initrd-image" \
		USE_DOCKER=1 \
		AGENT_INIT="yes"
	mv "kata-containers-initrd.img" "${install_dir}/${initrd_name}"
	(
		cd "${install_dir}"
		ln -sf "${builddir}" kata-containers-initrd.img
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
	(
		cd "${install_dir}"
		ln -sf "${image_name}" kata-containers.img
	)
}

usage() {
	return_code=${1:-0}
	cat <<EOT
Create image and initrd in a tarball for kata containers.
Use it to build an image to distribute kata.

Usage:
${script_name} [options]

Options:
 --imagetype=${image_type}
 --prefix=${prefix}
 --destdir=${destdir}
EOT

	exit "${return_code}"
}

main() {
	image_type=image
	destdir="$PWD"
	prefix="/opt/kata"
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
