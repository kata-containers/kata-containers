#!/usr/bin/env bash
#
# Copyright (c) 2025 Kata Contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Build a modules volume disk image from a modules tarball.
# The resulting image can be attached to a kata guest VM as a
# secondary block device so that kernel modules are available
# without modifying the dm-verity measured rootfs.
#
# Optionally creates a dm-verity hash partition for the modules
# volume itself (defense-in-depth alongside kernel module signing).
#
# SECURITY RISKS:
# - Without dm-verity (default), the modules volume has no
#   integrity protection beyond kernel module signing. An attacker
#   who can modify the volume image could replace modules, though
#   CONFIG_MODULE_SIG_FORCE will reject unsigned replacements.
# - With dm-verity (-V), the volume hash must be passed to the
#   runtime configuration. If the hash is not verified during
#   attestation, it provides no security benefit.
# - The volume image file permissions on the host must prevent
#   unauthorized modification (e.g., root-only, read-only).

set -o errexit
set -o nounset
set -o pipefail

usage() {
	cat <<EOF
Build a modules volume disk image for kata guest VMs.

Usage:
  $(basename "$0") [options]

Options:
  -m <path>     Path to modules tarball (required).
                Expected to contain lib/modules/<version>/ tree.
  -o <path>     Output directory (default: PWD).
  -V            Enable dm-verity on the modules volume.
                Writes verity params to modules_verity_params.txt.
  -h            Display this help.

Example:
  $(basename "$0") -m kata-modules-6.12.0-kata.tar.zst -o /tmp/
  $(basename "$0") -m kata-modules-6.12.0-kata.tar.zst -V -o /tmp/

EOF
	exit "${1:-0}"
}

die() {
	echo "ERROR: $*" >&2
	exit 1
}

info() {
	echo "INFO: $*"
}

modules_tarball=""
output_dir="${PWD}"
enable_verity="false"

while getopts "m:o:Vh" opt; do
	case "$opt" in
		m) modules_tarball="$(realpath "${OPTARG}")" ;;
		o) output_dir="${OPTARG}" ;;
		V) enable_verity="true" ;;
		h) usage 0 ;;
		*) usage 1 ;;
	esac
done

[ -n "${modules_tarball}" ] || die "Modules tarball is required (-m)"
[ -f "${modules_tarball}" ] || die "Modules tarball not found: ${modules_tarball}"

workdir="$(mktemp -d)"
trap 'rm -rf "${workdir}"' EXIT

info "Extracting modules tarball"
modules_dir="${workdir}/rootfs"
mkdir -p "${modules_dir}"

case "${modules_tarball}" in
	*.tar.zst|*.tar.zstd) tar --zstd -xf "${modules_tarball}" -C "${modules_dir}" ;;
	*.tar.gz|*.tgz)       tar -xzf "${modules_tarball}" -C "${modules_dir}" ;;
	*.tar.xz)             tar -xJf "${modules_tarball}" -C "${modules_dir}" ;;
	*.tar)                tar -xf "${modules_tarball}" -C "${modules_dir}" ;;
	*) die "Unsupported tarball format: ${modules_tarball}" ;;
esac

modules_size_kb=$(du -sk "${modules_dir}" | awk '{print $1}')
img_size_kb=$(( modules_size_kb + 4096 ))

mkdir -p "${output_dir}"
image="${output_dir}/kata-modules-volume.img"

info "Creating ext4 image (${img_size_kb} KiB)"
dd if=/dev/zero of="${image}" bs=1024 count="${img_size_kb}" status=none
mkfs.ext4 -q -F -L kata-modules -d "${modules_dir}" "${image}"

if [ "${enable_verity}" == "true" ]; then
	info "Setting up dm-verity for modules volume"

	command -v veritysetup >/dev/null 2>&1 || die "veritysetup not found; install cryptsetup-bin"

	data_size_bytes=$(stat -c%s "${image}")
	data_blocks_count=$(( data_size_bytes / 4096 ))

	hash_size_kb=$(( img_size_kb / 100 + 1024 ))
	total_size_kb=$(( img_size_kb + hash_size_kb ))

	truncate -s "${total_size_kb}K" "${image}"

	verity_output=$(veritysetup format \
		--data-block-size=4096 \
		--hash-block-size=4096 \
		--data-blocks="${data_blocks_count}" \
		--hash-offset="${data_size_bytes}" \
		"${image}" "${image}" 2>&1) || die "veritysetup format failed: ${verity_output}"

	root_hash=$(echo "${verity_output}" | sed -n 's/^Root hash:[[:space:]]*//p')
	salt=$(echo "${verity_output}" | sed -n 's/^Salt:[[:space:]]*//p')
	data_blocks=$(echo "${verity_output}" | sed -n 's/^Data blocks:[[:space:]]*//p')
	data_block_size=$(echo "${verity_output}" | sed -n 's/^Data block size:[[:space:]]*//p')
	hash_block_size=$(echo "${verity_output}" | sed -n 's/^Hash block size:[[:space:]]*//p')

	verity_params="root_hash=${root_hash},salt=${salt},data_blocks=${data_blocks},data_block_size=${data_block_size},hash_block_size=${hash_block_size}"

	printf '%s\n' "${verity_params}" > "${output_dir}/modules_verity_params.txt"
	info "Verity params written to: ${output_dir}/modules_verity_params.txt"
fi

info "Modules volume image created: ${image}"
info "Image size: $(du -h "${image}" | awk '{print $1}')"
