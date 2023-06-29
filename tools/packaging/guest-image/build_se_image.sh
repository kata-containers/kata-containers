#!/usr/bin/env bash
# Copyright (c) 2023 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

[ -n "${DEBUG:-}" ] && set -x

set -o errexit
set -o nounset
set -o pipefail

readonly script_name="$(basename "${BASH_SOURCE[0]}")"
readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly packaging_root_dir="$(cd "${script_dir}/../" && pwd)"
readonly kata_root_dir="$(cd "${packaging_root_dir}/../../" && pwd)"

source "$kata_root_dir/ci/lib.sh"
source "${packaging_root_dir}/scripts/lib.sh"

[ "$(uname -m)" = s390x ] || die "Building a Secure Execution image is currently only supported on s390x."

usage() {
	cat >&2 << EOF
Usage:
  ${script_name} [options]

Options:
  --builddir=${builddir}
  --destdir=${destdir}

Environment variables:
  HKD_PATH (required): Secure Execution host key document, generally specific to your machine. See
                  https://www.ibm.com/docs/en/linux-on-systems?topic=tasks-verify-host-key-document
                  for information on how to retrieve and verify this document.
  DEBUG         : If set, display debug information.
EOF
	exit "${1:-0}"
}

build_image() {
	image_source_dir="${builddir}/secure-image"
	mkdir -p "${image_source_dir}"
	pushd "${tarball_dir}"
	for tarball_id in cc-kernel cc-rootfs-initrd; do
		tar xvf kata-static-${tarball_id}.tar.xz -C "${image_source_dir}"
	done
	popd

	protimg_source_dir="${image_source_dir}${prefix}/share/kata-containers"
	local kernel_params="agent.enable_signature_verification=false"
	if ! build_secure_image "${kernel_params}" "${protimg_source_dir}" "${install_dir}"; then
		usage 1
	fi
}

main() {
	readonly prefix="/opt/confidential-containers"
	builddir="${PWD}"
	tarball_dir="${builddir}/../.."
	while getopts "h-:" opt; do
		case "$opt" in
		-)
			case "${OPTARG}" in
			builddir=*)
				builddir=${OPTARG#*=}
				;;
			destdir=*)
				destdir=${OPTARG#*=}
				;;
			*)
				echo >&2 "ERROR: Invalid option -$opt${OPTARG}"
				usage 1
				;;
			esac
			;;
		h) usage 0 ;;
		*)
			echo "Invalid option $opt" >&2
			usage 1
			;;
		esac
	done
	readonly destdir
	readonly builddir

	info "Build IBM zSystems & LinuxONE SE image"

	install_dir="${destdir}${prefix}/share/kata-containers"
	readonly install_dir

	mkdir -p "${install_dir}"

	build_image
}

main $*
