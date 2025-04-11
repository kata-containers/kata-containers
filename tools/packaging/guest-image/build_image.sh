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
export AGENT_TARBALL=${AGENT_TARBALL:-}

ARCH=${ARCH:-$(uname -m)}
if [ $(uname -m) == "${ARCH}" ]; then
       arch_target="$(uname -m)"
else
       arch_target="${ARCH}"
fi

final_artifact_name="kata-containers"
image_initrd_extension=".img"

build_initrd() {
	info "Build initrd"
	info "initrd os: $os_name"
	info "initrd os version: $os_version"
	make initrd \
		VARIANT="${image_initrd_suffix}" \
		DISTRO="$os_name" \
		DEBUG="${DEBUG:-}" \
		OS_VERSION="${os_version}" \
		ROOTFS_BUILD_DEST="${builddir}/initrd-image" \
		USE_DOCKER=1 \
		AGENT_TARBALL="${AGENT_TARBALL}" \
		AGENT_INIT="${AGENT_INIT:-no}" \
		AGENT_POLICY="${AGENT_POLICY:-}" \
		PULL_TYPE="${PULL_TYPE:-default}" \
		COCO_GUEST_COMPONENTS_TARBALL="${COCO_GUEST_COMPONENTS_TARBALL:-}" \
		PAUSE_IMAGE_TARBALL="${PAUSE_IMAGE_TARBALL:-}"

	if [[ "${image_initrd_suffix}" == "nvidia-gpu"* ]]; then
		nvidia_driver_version=$(cat "${builddir}"/initrd-image/*/nvidia_driver_version)
		artifact_name=${artifact_name/.initrd/"-${nvidia_driver_version}".initrd}
	fi

	mv -f "kata-containers-initrd.img" "${install_dir}/${artifact_name}"
	(
		cd "${install_dir}"
		ln -sf "${artifact_name}" "${final_artifact_name}${image_initrd_extension}"
	)
}

build_image() {
	info "Build image"
	info "image os: $os_name"
	info "image os version: $os_version"
	make image \
		VARIANT="${image_initrd_suffix}" \
		DISTRO="${os_name}" \
		DEBUG="${DEBUG:-}" \
		USE_DOCKER="1" \
		OS_VERSION="${os_version}" \
		ROOTFS_BUILD_DEST="${builddir}/rootfs-image" \
		AGENT_TARBALL="${AGENT_TARBALL}" \
		AGENT_POLICY="${AGENT_POLICY:-}" \
		PULL_TYPE="${PULL_TYPE:-default}" \
		COCO_GUEST_COMPONENTS_TARBALL="${COCO_GUEST_COMPONENTS_TARBALL:-}" \
		PAUSE_IMAGE_TARBALL="${PAUSE_IMAGE_TARBALL:-}"

	if [[ "${image_initrd_suffix}" == "nvidia-gpu"* ]]; then
		nvidia_driver_version=$(cat "${builddir}"/rootfs-image/*/nvidia_driver_version)
		artifact_name=${artifact_name/.image/"-${nvidia_driver_version}".image}
	fi

	mv -f "kata-containers.img" "${install_dir}/${artifact_name}"
	if [ -e "root_hash.txt" ]; then
	    cp root_hash.txt "${install_dir}/"
	fi
	(
		cd "${install_dir}"
		ln -sf "${artifact_name}" "${final_artifact_name}${image_initrd_extension}"
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
 --osname=\${os_name}
 --osversion=\${os_version}
 --imagetype=\${image_type}
 --prefix=\${prefix}
 --destdir=\${destdir}
 --image_initrd_suffix=\${image_initrd_suffix}
EOF

	exit "${return_code}"
}

main() {
	image_type=image
	destdir="$PWD"
	prefix="/opt/kata"
	image_suffix=""
	image_initrd_suffix=""
	builddir="${PWD}"
	while getopts "h-:" opt; do
		case "$opt" in
		-)
			case "${OPTARG}" in
			osname=*)
				os_name=${OPTARG#*=}
				;;
			osversion=*)
				os_version=${OPTARG#*=}
				;;
			imagetype=image)
				image_type=image
				;;
			imagetype=initrd)
				image_type=initrd
				;;
			image_initrd_suffix=*)
				image_initrd_suffix=${OPTARG#*=}
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

	if [ "${image_type}" = "initrd" ]; then
		final_artifact_name+="-initrd"
	fi

	if [ -n "${image_initrd_suffix}" ]; then
		artifact_name="kata-${os_name}-${os_version}-${image_initrd_suffix}.${image_type}"
		final_artifact_name+="-${image_initrd_suffix}"
	else
		artifact_name="kata-${os_name}-${os_version}.${image_type}"
	fi

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
