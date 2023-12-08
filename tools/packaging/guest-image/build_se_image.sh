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

ARCH=${ARCH:-$(uname -m)}
if [ $(uname -m) == "${ARCH}" ]; then
	[ "${ARCH}" == "s390x" ] || die "Building a Secure Execution image is currently only supported on s390x."
fi

finish() {
	if [ -e "${parmfile}" ]; then
		rm -f "${parmfile}"
	fi
}

trap finish EXIT

usage() {
	cat >&2 << EOF
Usage:
  ${script_name} [options]

Options:
  --builddir=${builddir}
  --destdir=${destdir}

Environment variables:
  HKD_PATH (required): a path for a directory which includes at least one host key document
                  for Secure Execution, generally specific to your machine. See
                  https://www.ibm.com/docs/en/linux-on-systems?topic=tasks-verify-host-key-document
                  for information on how to retrieve and verify this document.
  SIGNING_KEY_CERT_PATH: a path for the IBM zSystem signing key certificate
  INTERMEDIATE_CA_CERT_PATH: a path for the intermediate CA certificate signed by the root CA
  DEBUG         : If set, display debug information.
EOF
	exit "${1:-0}"
}

# Build a IBM zSystem secure execution (SE) image
#
# Parameters:
#	$1	- kernel_parameters
#	$2	- a source directory where kernel and initrd are located
#	$3	- a destination directory where a SE image is built
#
# Return:
# 	0 if the image is successfully built
#	1 otherwise
build_secure_image() {
	kernel_params="${1:-}"
	install_src_dir="${2:-}"
	install_dest_dir="${3:-}"
	key_verify_option="--no-verify" # no verification for CI testing purposes

	if [ -n "${SIGNING_KEY_CERT_PATH:-}" ] && [ -n "${INTERMEDIATE_CA_CERT_PATH:-}" ]; then
		if [ -e "${SIGNING_KEY_CERT_PATH}" ] && [ -e "${INTERMEDIATE_CA_CERT_PATH}" ]; then
			key_verify_option="--cert=${SIGNING_KEY_CERT_PATH} --cert=${INTERMEDIATE_CA_CERT_PATH}"
		else
			die "Specified certificate(s) not found"
		fi
	fi

	if [ ! -f "${install_src_dir}/vmlinuz.container" ] ||
		[ ! -f "${install_src_dir}/kata-containers-initrd.img" ]; then
		cat << EOF >&2
Either kernel or initrd does not exist or is mistakenly named
A file name for kernel must be vmlinuz.container (raw binary)
A file name for initrd must be kata-containers-initrd.img
EOF
		return 1
	fi

	cmdline="${kernel_params} panic=1 scsi_mod.scan=none swiotlb=262144"
	parmfile="$(mktemp --suffix=-cmdline)"
	echo "${cmdline}" > "${parmfile}"
	chmod 600 "${parmfile}"

	[ -n "${HKD_PATH:-}" ] || (echo >&2 "No host key document specified." && return 1)
	cert_list=($(ls -1 $HKD_PATH))
	declare hkd_options
	eval "for cert in ${cert_list[*]}; do
		hkd_options+=\"--host-key-document=\\\"\$HKD_PATH/\$cert\\\" \"
	done"

	command -v genprotimg > /dev/null 2>&1 || die "A package s390-tools is not installed."
	extra_arguments=""
	genprotimg_version=$(genprotimg --version | grep -Po '(?<=version )[^-]+')
	if ! version_greater_than_equal "${genprotimg_version}" "2.17.0"; then
		extra_arguments="--x-pcf '0xe0'"
	fi

	eval genprotimg \
		"${extra_arguments}" \
		"${hkd_options}" \
		--output="${install_dest_dir}/kata-containers-se.img" \
		--image="${install_src_dir}/vmlinuz.container" \
		--ramdisk="${install_src_dir}/kata-containers-initrd.img" \
		--parmfile="${parmfile}" \
		"${key_verify_option}"

	build_result=$?
	if [ $build_result -eq 0 ]; then
		return 0
	else
		return 1
	fi
}

build_image() {
	image_source_dir="${builddir}/secure-image"
	mkdir -p "${image_source_dir}"
	pushd "${tarball_dir}"
	for tarball_id in kernel rootfs-initrd; do
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

	info "Build IBM zSystems & LinuxONE SE image"

	install_dir="${destdir}${prefix}/share/kata-containers"
	readonly install_dir

	mkdir -p "${install_dir}"

	build_image
}

main $*
