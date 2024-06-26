#!/usr/bin/env bash
# Copyright (c) 2024 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

set -o nounset

readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly packaging_root_dir="$(cd "${script_dir}/../" && pwd)"
readonly kata_root_dir="$(cd "${packaging_root_dir}/../../" && pwd)"

source "$kata_root_dir/tests/common.bash"

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

	if [ ! -f "${install_src_dir}/vmlinuz-confidential.container" ] ||
		[ ! -f "${install_src_dir}/kata-containers-initrd-confidential.img" ]; then
		cat << EOF >&2
Either kernel or initrd does not exist or is mistakenly named
A file name for kernel must be vmlinuz-confidential.container (raw binary)
A file name for initrd must be kata-containers-initrd-confidential.img
EOF
		return 1
	fi

	cmdline="${kernel_params} panic=1 scsi_mod.scan=none swiotlb=262144 agent.debug_console agent.debug_console_vport=1026"
	parmfile="$(mktemp --suffix=-cmdline)"
	echo "${cmdline}" > "${parmfile}"
	chmod 600 "${parmfile}"

	[ -n "${HKD_PATH:-}" ] || (echo >&2 "No host key document specified." && return 1)
	cert_list=($(ls -1 $HKD_PATH/HKD-*.crt | xargs -n 1 basename))
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
		--image="${install_src_dir}/vmlinuz-confidential.container" \
		--ramdisk="${install_src_dir}/kata-containers-initrd-confidential.img" \
		--parmfile="${parmfile}" \
		"${key_verify_option}"

	build_result=$?
	rm -f "${parmfile}"
	if [ $build_result -eq 0 ]; then
		return 0
	else
		return 1
	fi
}
