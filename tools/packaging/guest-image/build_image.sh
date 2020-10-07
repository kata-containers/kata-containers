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
readonly tmp_dir=$(mktemp -d -t build-image-tmp.XXXXXXXXXX)
export   GOPATH="${tmp_dir}/go"

export GOPATH=${GOPATH:-${HOME}/go}
source "${packaging_root_dir}/scripts/lib.sh"

exit_handler() {
	[ -d "${tmp_dir}" ] && sudo rm -rf "$tmp_dir"
}
trap exit_handler EXIT

arch_target="$(uname -m)"

source "${packaging_root_dir}/versions.txt"

readonly destdir="${PWD}"

build_initrd() {
	sudo -E PATH="$PATH" make initrd \
		DISTRO="$initrd_distro" \
		DEBUG="${DEBUG:-}" \
		OS_VERSION="${initrd_os_version}" \
		ROOTFS_BUILD_DEST="${tmp_dir}/initrd-image" \
		USE_DOCKER=1 \
		AGENT_INIT="yes"
}

build_image() {
	sudo -E PATH="${PATH}" make image \
		DISTRO="${img_distro}" \
		DEBUG="${DEBUG:-}" \
		USE_DOCKER="1" \
		IMG_OS_VERSION="${img_os_version}" \
		ROOTFS_BUILD_DEST="${tmp_dir}/rootfs-image"
}

create_tarball() {
	agent_sha=$(get_repo_hash "${script_dir}")
	#reduce sha size for short names
	agent_sha=${agent_sha:0:${short_commit_length}}
	tarball_name="kata-containers-${kata_version}-${agent_sha}-${arch_target}.tar.gz"
	image_name="kata-containers-image_${img_distro}_${kata_version}_agent_${agent_sha}.img"
	initrd_name="kata-containers-initrd_${initrd_distro}_${kata_version}_agent_${agent_sha}.initrd"

	mv "${osbuilder_dir}/kata-containers.img" "${image_name}"
	mv "${osbuilder_dir}/kata-containers-initrd.img" "${initrd_name}"
	sudo tar cfzv "${tarball_name}" "${initrd_name}" "${image_name}"
}

usage() {
	return_code=${1:-0}
	cat <<EOT
Create image and initrd in a tarball for kata containers.
Use it to build an image to distribute kata.

Usage:
${script_name} [options]

Options:
 -v <version> : Kata version to build images. Use kata release for
                 for agent and osbuilder.

EOT

	exit "${return_code}"
}

main() {
	while getopts "v:h" opt; do
		case "$opt" in
		h) usage 0 ;;
		v) kata_version="${OPTARG}" ;;
		*)
			echo "Invalid option $opt"
			usage 1
			;;
		esac
	done

	install_yq

	#image information
	img_distro=$(get_from_kata_deps "assets.image.architecture.${arch_target}.name" "${kata_version}")
	#In old branches this is not defined, use a default
	img_distro=${img_distro:-clearlinux}
	img_os_version=$(get_from_kata_deps "assets.image.architecture.${arch_target}.version" "${kata_version}")

	#initrd information
	initrd_distro=$(get_from_kata_deps "assets.initrd.architecture.${arch_target}.name" "${kata_version}")
	#In old branches this is not defined, use a default
	initrd_distro=${initrd_distro:-alpine}
	initrd_os_version=$(get_from_kata_deps "assets.image.architecture.${arch_target}.version" "${kata_version}")

	shift "$((OPTIND - 1))"
	pushd "${osbuilder_dir}"
	build_initrd
	build_image
	create_tarball
	cp "${tarball_name}" "${destdir}"
	popd
}

main $*
