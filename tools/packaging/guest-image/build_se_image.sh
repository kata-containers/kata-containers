#!/usr/bin/env bash
# Copyright (c) 2023 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

[[ -n "${DEBUG:-}" ]] && set -x

set -o errexit
set -o nounset
set -o pipefail

script_name="$(basename "${BASH_SOURCE[0]}")"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
packaging_root_dir="$(cd "${script_dir}/../" && pwd)"
# shellcheck disable=SC2034
kata_root_dir="$(cd "${packaging_root_dir}/../../" && pwd)"

# shellcheck source=/dev/null
source "${packaging_root_dir}/scripts/lib.sh"
# shellcheck source=/dev/null
source "${script_dir}/lib_se.sh"

ARCH=${ARCH:-$(uname -m)}
if [[ "${FAKE_SE_IMAGE:-}" != "true" && "$(uname -m)" == "${ARCH}" ]]; then
	[[ "${ARCH}" == "s390x" ]] || die "Building a Secure Execution image is currently only supported on s390x."
fi
usage() {
	cat >&2 << EOF
Usage:
  ${script_name} [options]

Options:
  --builddir=\${builddir}
  --composable
  --destdir=\${destdir}

Environment variables:
  SE_COMPOSABLE    : If set to "yes", builds a composable SE image using a standard initrd (runtime-rs based).
                     If set to "no" (default), builds a standalone confidential SE image using a confidential initrd.
                     Set automatically from the --composable flag; override only if calling lib_se.sh directly.
  HKD_PATH (required unless FAKE_SE_IMAGE=true): a path for a directory which includes at least one host key document
                  for Secure Execution, generally specific to your machine. See
                  https://www.ibm.com/docs/en/linux-on-systems?topic=tasks-verify-host-key-document
                  for information on how to retrieve and verify this document.
  SIGNING_KEY_CERT_PATH: a path for the IBM zSystem signing key certificate
  INTERMEDIATE_CA_CERT_PATH: a path for the intermediate CA certificate signed by the root CA
  HOST_KEY_CRL_PATH: a path for the host key CRL
  FAKE_SE_IMAGE : If set to "true", creates a dummy SE image via touch command
                  instead of using genprotimg. Useful for testing without real SE setup.
  DEBUG         : If set, display debug information.
EOF
	exit "${1:-0}"
}

build_image() {
	# Check if FAKE_SE_IMAGE mode is enabled
	if [[ "${FAKE_SE_IMAGE:-}" == "true" ]]; then
		echo "FAKE_SE_IMAGE mode enabled: Skipping tarball extraction"
		if ! build_secure_image "" "" "${install_dir}"; then
			usage 1
		fi
		return 0
	fi

	image_source_dir="${builddir}/secure-image"
	mkdir -p "${image_source_dir}"
	pushd "${tarball_dir}"
	if [[ "${SE_COMPOSABLE}" == "yes" ]]; then
		initrd_tarball_id="rootfs-initrd"
	else
		initrd_tarball_id="rootfs-initrd-confidential"
	fi
	for tarball_id in kernel "${initrd_tarball_id}"; do
		tar --zstd -xvf "kata-static-${tarball_id}.tar.zst" -C "${image_source_dir}"
	done

	# For the composable path, extract the CoCo extension root hash so the
	# sealed SE kernel cmdline carries the verity params.
	# COCO_VERITY_PARAMS must be set; lib_se.sh will die if it is absent.
	if [[ "${SE_COMPOSABLE}" == "yes" ]]; then
		local coco_ext_tarball="kata-static-rootfs-image-coco-extension.tar.zst"
		local root_hash_path="./opt/kata/share/kata-containers/root_hash_coco-extension.txt"
		tar --zstd -tf "${coco_ext_tarball}" "${root_hash_path}" >/dev/null 2>&1 \
			|| die "root_hash_coco-extension.txt not found in ${coco_ext_tarball}"
		local root_hash_tmp
		root_hash_tmp="$(tar --zstd -xOf "${coco_ext_tarball}" "${root_hash_path}")"
		root_hash_tmp="${root_hash_tmp%$'\r'}"
		[[ -n "${root_hash_tmp}" ]] || die "Empty root_hash_coco-extension.txt in ${coco_ext_tarball}"
		export COCO_VERITY_PARAMS="${root_hash_tmp}"
	fi
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
	export SE_COMPOSABLE="no"
	while getopts "h-:" opt; do
		case "${opt}" in
		-)
			case "${OPTARG}" in
			builddir=*)
				builddir=${OPTARG#*=}
				;;
			composable)
				export SE_COMPOSABLE="yes"
				;;
			destdir=*)
				destdir=${OPTARG#*=}
				;;
			*)
				echo >&2 "ERROR: Invalid option -${opt}${OPTARG}"
				usage 1
				;;
			esac
			;;
		h) usage 0 ;;
		*)
			echo "Invalid option ${opt}" >&2
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

main "$@"
