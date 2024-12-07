#!/usr/bin/env bash
# Copyright (c) 2023 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

[ -n "${DEBUG:-}" ] && set -x

set -o errexit
set -o nounset
set -o pipefail

script_name="$(basename "${BASH_SOURCE[0]}")"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
packaging_root_dir="$(cd "${script_dir}/../" && pwd)"
kata_root_dir="$(cd "${packaging_root_dir}/../../" && pwd)"

source "${packaging_root_dir}/scripts/lib.sh"
source "${script_dir}/lib_se.sh"

ARCH=${ARCH:-$(uname -m)}
if [ $(uname -m) == "${ARCH}" ]; then
	[ "${ARCH}" == "s390x" ] || die "Building a Secure Execution image is currently only supported on s390x."
fi
usage() {
	cat >&2 << EOF
Usage:
  ${script_name} [options]

Options:
  --builddir=\${builddir}
  --destdir=\${destdir}

Environment variables:
  HKD_PATH (required): a path for a directory which includes at least one host key document
                  for Secure Execution, generally specific to your machine. See
                  https://www.ibm.com/docs/en/linux-on-systems?topic=tasks-verify-host-key-document
                  for information on how to retrieve and verify this document.
  SIGNING_KEY_CERT_PATH: a path for the IBM zSystem signing key certificate
  INTERMEDIATE_CA_CERT_PATH: a path for the intermediate CA certificate signed by the root CA
  HOST_KEY_CRL_PATH: a path for the host key CRL
  DEBUG         : If set, display debug information.
EOF
	exit "${1:-0}"
}

build_image() {
	image_source_dir="${builddir}/secure-image"
	mkdir -p "${image_source_dir}"
	pushd "${tarball_dir}"
	for tarball_id in kernel-confidential rootfs-initrd-confidential; do
		tar xvf kata-static-${tarball_id}.tar.xz -C "${image_source_dir}"
	done
	popd

	protimg_source_dir="${image_source_dir}${prefix}/share/kata-containers"
	local kernel_params="${SE_KERNEL_PARAMS:-}"
	if ! build_secure_image "${kernel_params}" "${protimg_source_dir}" "${install_dir}"; then
		usage 1
	fi
}

main() {
	readonly prefix="/opt/kata"
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

	info "Build IBM zSystems & LinuxONE Secure Execution(SE) image"

	install_dir="${destdir}${prefix}/share/kata-containers"
	readonly install_dir

	mkdir -p "${install_dir}"

	build_image
}

main $*
