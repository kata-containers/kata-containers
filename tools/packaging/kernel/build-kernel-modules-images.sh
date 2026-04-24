#!/usr/bin/env bash
#
# Copyright (c) 2026 Kata Contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Build kernel module disk images for kata guest VMs.
#
# This script:
# 1. Invokes build-kernel.sh to compile the kernel with extra module
#    config fragments (e.g., MLNX, NTFS3).
# 2. Runs modules_install to collect all built modules.
# 3. Filters modules by subsystem into per-set staging directories.
# 4. Packages each set into a disk image using build-modules-volume.sh.
#
# The kernel binary itself is discarded; only the module images are
# the desired output.

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

packaging_scripts_dir="${script_dir}/../scripts"
# shellcheck source=tools/packaging/scripts/lib.sh
source "${packaging_scripts_dir}/lib.sh"

readonly build_kernel="${script_dir}/build-kernel.sh"
readonly build_modules_volume="${script_dir}/build-modules-volume.sh"

usage() {
	cat <<EOF
Build kernel module disk images for kata guest VMs.

Usage:
  $(basename "$0") [options]

Options:
  -a <arch>     Target architecture (default: host arch).
  -o <dir>      Output directory for module images (default: PWD).
  -V            Enable dm-verity on module images.
  -h            Display this help.

The script builds the kernel with all module-set config fragments,
then splits the resulting modules into per-set disk images:

  - mlx5:  Mellanox MLX5 + InfiniBand drivers

EOF
	exit "${1:-0}"
}

# Module sets: name -> array of source paths under lib/modules/<ver>/kernel/
# that belong to this set.
declare -A MODULE_SETS
MODULE_SETS[mlx5]="drivers/net/ethernet/mellanox drivers/infiniband"

# Module set config fragments (relative to fragments/modules/)
declare -A MODULE_FRAGMENTS
MODULE_FRAGMENTS[mlx5]="mlx5.conf"

output_dir="${PWD}"
enable_verity="false"
arch_target=""

while getopts "a:o:Vh" opt; do
	case "${opt}" in
		a) arch_target="${OPTARG}" ;;
		o) output_dir="${OPTARG}" ;;
		V) enable_verity="true" ;;
		h) usage 0 ;;
		*) usage 1 ;;
	esac
done

[[ -n "${arch_target}" ]] || arch_target="$(uname -m)"

fragments_dir="${script_dir}/configs/fragments/modules"
for name in "${!MODULE_FRAGMENTS[@]}"; do
	frag="${fragments_dir}/${MODULE_FRAGMENTS[${name}]}"
	[[ -f "${frag}" ]] || die "Config fragment not found: ${frag}"
done

kernel_version=$(get_from_kata_deps ".assets.kernel.version")
kernel_version="${kernel_version#v}"
config_version=$(cat "${script_dir}/kata_config_version")
kernel_path="${PWD}/kata-linux-modules-${kernel_version}-${config_version}"

info "Building kernel ${kernel_version} with module configs"

# Collect all module fragment paths as extra config files for build-kernel.sh
extra_configs=""
for name in "${!MODULE_FRAGMENTS[@]}"; do
	extra_configs="${extra_configs} ${fragments_dir}/${MODULE_FRAGMENTS[${name}]}"
done

# Build the kernel with module support + module signing + extra module fragments.
# We use -x (confidential) so that modules.conf and module_signing.conf are
# included, and KBUILD_SIGN_PIN (if set) is used to sign the modules.
# The -s flag skips redundant config checks since our module fragments may
# overlap with configs already set by the confidential build.
"${build_kernel}" \
	-a "${arch_target}" \
	-v "${kernel_version}" \
	-k "${kernel_path}" \
	-x \
	-s \
	-f \
	setup

# Append module fragment configs to the generated .config and re-run olddefconfig
arch_kernel=$(case "${arch_target}" in
	x86_64) echo "x86_64" ;;
	aarch64) echo "arm64" ;;
	s390x) echo "s390" ;;
	ppc64le|powerpc64) echo "powerpc" ;;
	*) echo "${arch_target}" ;;
esac)

arch_frag_dir="${script_dir}/configs/fragments/${arch_kernel}"
config_path="${arch_frag_dir}/.config"

info "Merging module config fragments into kernel config"
for name in "${!MODULE_FRAGMENTS[@]}"; do
	frag="${fragments_dir}/${MODULE_FRAGMENTS[${name}]}"
	info "  Appending: ${frag}"
	cat "${frag}" >> "${config_path}"
done

cp "${config_path}" "${kernel_path}/.config"
make -C "${kernel_path}" ARCH="${arch_kernel}" olddefconfig

info "Building kernel"
make -C "${kernel_path}" -j "$(nproc)" ARCH="${arch_kernel}"

info "Installing modules"
make -C "${kernel_path}" -j "$(nproc)" \
	INSTALL_MOD_STRIP=1 \
	INSTALL_MOD_PATH="${kernel_path}" \
	modules_install

# Find the installed modules version directory
modules_base="${kernel_path}/lib/modules"
mod_version=$(ls "${modules_base}" | head -1)
modules_tree="${modules_base}/${mod_version}"

[[ -d "${modules_tree}/kernel" ]] || die "No modules installed at ${modules_tree}/kernel"

info "Modules installed at ${modules_tree}"

mkdir -p "${output_dir}"

for name in "${!MODULE_SETS[@]}"; do
	paths="${MODULE_SETS[${name}]}"
	staging=$(mktemp -d)
	staging_modules="${staging}/lib/modules/${mod_version}"
	mkdir -p "${staging_modules}/kernel"

	has_modules="false"
	for subpath in ${paths}; do
		src="${modules_tree}/kernel/${subpath}"
		if [[ -d "${src}" ]]; then
			dst="${staging_modules}/kernel/${subpath}"
			mkdir -p "$(dirname "${dst}")"
			cp -a "${src}" "${dst}"
			has_modules="true"
		fi
	done

	if [[ "${has_modules}" == "false" ]]; then
		info "No modules found for set '${name}', skipping"
		rm -rf "${staging}"
		continue
	fi

	# Copy metadata files needed by depmod
	for f in modules.order modules.builtin; do
		[[ -f "${modules_tree}/${f}" ]] && cp "${modules_tree}/${f}" "${staging_modules}/"
	done

	# Generate modules.dep and related files for this subset
	depmod -b "${staging}" "${mod_version}"

	info "Creating intermediate tarball for module set '${name}'"
	tarball=$(mktemp --suffix=".tar.gz")
	tar -czf "${tarball}" -C "${staging}" .

	info "Building disk image for module set '${name}'"
	verity_flag=""
	[[ "${enable_verity}" == "true" ]] && verity_flag="-V"
	# shellcheck disable=SC2086
	"${build_modules_volume}" -m "${tarball}" -o "${output_dir}" ${verity_flag}
	rm -f "${tarball}"

	# Rename generic output to per-set name
	mv "${output_dir}/kata-modules-volume.img" "${output_dir}/kata-modules-${name}.img"
	if [[ "${enable_verity}" == "true" ]] && [[ -f "${output_dir}/modules_verity_params.txt" ]]; then
		mv "${output_dir}/modules_verity_params.txt" "${output_dir}/kata-modules-${name}-verity-params.txt"
	fi

	info "Module image created: ${output_dir}/kata-modules-${name}.img"
	rm -rf "${staging}"
done

info "All module images built successfully"
