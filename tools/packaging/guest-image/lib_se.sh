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

	if [ -n "${SIGNING_KEY_CERT_PATH:-}" ] && [ -n "${INTERMEDIATE_CA_CERT_PATH:-}" ] && [ -n "${HOST_KEY_CRL_PATH:-}" ]; then
		if [ -e "${SIGNING_KEY_CERT_PATH}" ] && [ -e "${INTERMEDIATE_CA_CERT_PATH}" ] && [ -e "${HOST_KEY_CRL_PATH}" ]; then
			key_verify_option="--cert=${SIGNING_KEY_CERT_PATH} --cert=${INTERMEDIATE_CA_CERT_PATH} --crl=${HOST_KEY_CRL_PATH}"
		else
			die "Specified certificate(s) not found"
		fi
	elif [ -n "${SIGNING_KEY_CERT_PATH:-}" ] || [ -n "${INTERMEDIATE_CA_CERT_PATH:-}" ] || [ -n "${HOST_KEY_CRL_PATH:-}" ]; then
		die "All of SIGNING_KEY_CERT_PATH, INTERMEDIATE_CA_CERT_PATH, and HOST_KEY_CRL_PATH must be specified"
	else
		echo "No certificate specified. Using --no-verify option"
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

function repack_secure_image() {
	kernel_params_value="${1:-}"
	build_dir="${2:-}"
	for_kbs="${3:-false}"
	if [ -z "${build_dir}" ]; then
		>&2 echo "ERROR: build_dir for secure image is not specified"
		return 1
	fi
	config_file_path="/opt/kata/share/defaults/kata-containers/configuration-qemu-se.toml"
	if [ ! -f "${config_file_path}" ]; then
		>&2 echo "ERROR: config file not found: ${config_file_path}"
		return 1
	fi
	kernel_base_dir=$(dirname $(kata-runtime --config ${config_file_path} env --json | jq -r '.Kernel.Path'))
	# Make sure ${build_dir}/hdr exists
	mkdir -p "${build_dir}/hdr"
	# Prepare required files for building the secure image
	cp "${kernel_base_dir}/vmlinuz-confidential.container" "${build_dir}/hdr/"
	cp "${kernel_base_dir}/kata-containers-initrd-confidential.img" "${build_dir}/hdr/"
	# Build the secure image
	build_secure_image "${kernel_params_value}" "${build_dir}/hdr" "${build_dir}/hdr"
	# Get the secure image updated back to the kernel base directory
	if [ ! -f "${build_dir}/hdr/kata-containers-se.img" ]; then
		>&2 echo "ERROR: secure image not found: ${build_dir}/hdr/kata-containers-se.img"
		return 1
	fi
	sudo cp "${build_dir}/hdr/kata-containers-se.img" "${kernel_base_dir}/"
	if [ "${for_kbs}" == "true" ]; then
		# Rename kata-containers-se.img to hdr.bin and clean up kernel and initrd
		mv "${build_dir}/hdr/kata-containers-se.img" "${build_dir}/hdr/hdr.bin"
		rm -f ${build_dir}/hdr/{vmlinuz-confidential.container,kata-containers-initrd-confidential.img}
	else
		# Clean up the build directory completely
		rm -rf "${build_dir}"
	fi
}
