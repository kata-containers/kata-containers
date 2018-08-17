#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

description="
Description: This script is the *ONLY* to build a kernel for development.
"

set -o errexit
set -o nounset
set -o pipefail

readonly script_name="$(basename "${BASH_SOURCE[0]}")"
readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

#project_name
readonly project_name="kata-containers"
[ -n "${GOPATH:-}" ] || GOPATH="${HOME}/go"
# Fetch the first element from GOPATH as working directory
# as go get only works against the first item in the GOPATH
GOPATH="${GOPATH%%:*}"
# Kernel version to be used
kernel_version=""
# Flag know if need to download the kernel source
download_kernel=false
# The repository where kernel configuration lives
runtime_repository="github.com/${project_name}/runtime"
# The repository where kernel configuration lives
readonly kernel_config_repo="github.com/${project_name}/packaging"
readonly patches_repo="github.com/${project_name}/packaging"
readonly patches_repo_dir="${GOPATH}/src/${patches_repo}"
# Default path to search patches to apply to kernel
readonly default_patches_dir="${patches_repo_dir}/kernel/patches/"
# Default path to search config for kata
readonly default_kernel_config_dir="${GOPATH}/src/${kernel_config_repo}/kernel/configs"
#Path to kernel directory
kernel_path=""
#
patches_path=""
#
hypervisor_target=""
#
arch_target=""
#
kernel_config_path=""
# destdir
DESTDIR="${DESTDIR:-/}"
#PREFIX=
PREFIX="${PREFIX:-/usr}"

source "${script_dir}/../scripts/lib.sh"

usage() {
	cat <<EOT
Overview:

	Build a kernel for Kata Containers
	${description}

Usage:

	$script_name [options] <command> <argument>

Commands:

- setup

- build

- install

Options:

	-c <path>: Path to config file to build a the kernel
	-h       : Display this help.
	-k <path>: Path to kernel to build
	-p <path>: Path to a directory with patches to apply to kernel.
	-v       : Kernel version to use if kernel path not provided.
EOT
}

# Convert architecture to the name used by the Linux kernel build system
arch_to_kernel() {
	local -r arch="$1"

	case "$arch" in
	aarch64) echo "arm64" ;;
	ppc64le) echo "powerpc" ;;
	x86_64) echo "$arch" ;;
	*) die "unsupported architecture: $arch" ;;
	esac
}

get_kernel() {
	local version="${1:-}"
	#Remove extra 'v'
	version=${version#v}

	local kernel_path=${2:-}
	[ -n "${kernel_path}" ] || die "kernel_path not provided"
	[ ! -d "${kernel_path}" ] || die "kernel_path already exist"

	major_version=$(echo "${version}" | cut -d. -f1)
	kernel_tarball="linux-${version}.tar.xz"

	curl --fail -OL "https://cdn.kernel.org/pub/linux/kernel/v${major_version}.x/sha256sums.asc"
	grep "${kernel_tarball}" sha256sums.asc >"${kernel_tarball}.sha256"

	if [ -f "${kernel_tarball}" ] && ! sha256sum -c "${kernel_tarball}.sha256"; then
		info "invalid kernel tarball ${kernel_tarball} removing "
		rm -f "${kernel_tarball}"
	fi
	if [ ! -f "${kernel_tarball}" ]; then
		info "Download kernel version ${version}"
		info "Download kernel"
		curl --fail -OL "https://www.kernel.org/pub/linux/kernel/v${major_version}.x/${kernel_tarball}"
	else
		info "kernel tarball already downloaded"
	fi

	sha256sum -c "${kernel_tarball}.sha256"

	tar xf ${kernel_tarball}

	mv "linux-${version}" "${kernel_path}"
}

get_default_kernel_config() {
	local version="${1}"

	local hypervisor="$2"
	local kernel_arch="$3"

	[ -n "${version}" ] || die "kernel version not provided"
	[ -n "${hypervisor}" ] || die "hypervisor not provided"
	[ -n "${kernel_arch}" ] || die "kernel arch not provided"

	major_version=$(echo "${version}" | cut -d. -f1)
	minor_version=$(echo "${version}" | cut -d. -f2)
	config="${default_kernel_config_dir}/${kernel_arch}_kata_${hypervisor}_${major_version}.${minor_version}.x"
	[ -f "${config}" ] || die "failed to find default config ${config}"
	echo "${config}"
}

get_config_version() {
	config_version_file="${default_patches_dir}/../kata_config_version"
	if [ -f "${config_version_file}" ]; then
		cat "${config_version_file}"
	else
		die "failed to find ${config_version_file}"
	fi
}

setup_kernel() {
	local kernel_path=${1:-}
	[ -n "${kernel_path}" ] || die "kernel_path not provided"
	if [ -d "$kernel_path" ]; then
		info "${kernel_path} already exist"
		return
	fi

	info "kernel path does not exist, will download kernel"
	download_kernel="true"
	[ -n "$kernel_version" ] || die "failed to get kernel version: Kernel version is emtpy"

	if [[ ${download_kernel} == "true" ]]; then
		get_kernel "${kernel_version}" "${kernel_path}"
	fi

	[ -n "$kernel_path" ] || die "failed to find kernel source path"

	if [ -z "${patches_path}" ]; then
		patches_path="${default_patches_dir}"
		[ -d "${patches_path}" ] || git clone "https://${patches_repo}.git" "${patches_repo_dir}"
	fi

	[ -d "${patches_path}" ] || die " patches path '${patches_path}' does not exist"

	kernel_patches=$(find "${patches_path}" -name '*.patch' -type f)

	pushd "${kernel_path}" >>/dev/null
	for p in ${kernel_patches}; do
		info "Applying patch $p"
		patch -p1 <"$p"
	done

	[ -n "${hypervisor_target}" ] || hypervisor_target="kvm"
	[ -n "${arch_target}" ] || arch_target="$(uname -m)"
	arch_target=$(arch_to_kernel "${arch_target}")
	[ -n "${kernel_config_path}" ] || kernel_config_path=$(get_default_kernel_config "${kernel_version}" "${hypervisor_target}" "${arch_target}")

	cp "${kernel_config_path}" ./.config
	make oldconfig
}

build_kernel() {
	local kernel_path=${1:-}
	[ -n "${kernel_path}" ] || die "kernel_path not provided"
	[ -d "${kernel_path}" ] || die "path to kernel does not exist, use ${script_name} setup"
	[ -n "${arch_target}" ] || arch_target="$(arch)"
	arch_target=$(arch_to_kernel "${arch_target}")
	pushd "${kernel_path}" >>/dev/null
	make -j $(nproc) ARCH="${arch_target}"
	[ "$arch_target" != "powerpc" ] && ([ -e "arch/${arch_target}/boot/bzImage" ] || [ -e "arch/${arch_target}/boot/Image.gz" ])
	[ -e "vmlinux" ]
	popd >>/dev/null
}

install_kata() {
	local kernel_path=${1:-}
	[ -n "${kernel_path}" ] || die "kernel_path not provided"
	[ -d "${kernel_path}" ] || die "path to kernel does not exist, use ${script_name} setup"
	pushd "${kernel_path}" >>/dev/null
	config_version=$(get_config_version)
	[ -n "${config_version}" ] || die "failed to get config version"
	install_path=$(readlink -m "${DESTDIR}/${PREFIX}/share/${project_name}")
	vmlinuz="vmlinuz-${kernel_version}-${config_version}"
	vmlinux="vmlinux-${kernel_version}-${config_version}"

	if [ -e "arch/${arch_target}/boot/bzImage" ]; then
		bzImage="arch/${arch_target}/boot/bzImage"
	elif [ -e "arch/${arch_target}/boot/Image.gz" ]; then
		bzImage="arch/${arch_target}/boot/Image.gz"
	elif [ "${arch_target}" != "powerpc" ]; then
		die "failed to find image"
	fi

	if [ "${arch_target}" = "powerpc" ]; then
		install --mode 0644 -D "vmlinux" "${install_path}/${vmlinuz}"
	else
		install --mode 0644 -D "${bzImage}" "${install_path}/${vmlinuz}"
	fi

	install --mode 0644 -D "vmlinux" "${install_path}/${vmlinux}"
	install --mode 0644 -D ./.config "${install_path}/config-${kernel_version}"
	ln -sf "${vmlinuz}" "${install_path}/vmlinuz.container"
	ln -sf "${vmlinux}" "${install_path}/vmlinux.container"
	ls -la "${install_path}/vmlinux.container"
	ls -la "${install_path}/vmlinuz.container"
	popd >>/dev/null
}

main() {
	while getopts "a:c:hk:p:t:v:" opt; do
		case "$opt" in
		a)
			arch_target="${OPTARG}"
			;;
		c)
			kernel_config_path="${OPTARG}"
			;;

		h)
			usage
			exit 0
			;;

		k)
			kernel_path="${OPTARG}"
			;;

		t)
			hypervisor_target="${OPTARG}"
			;;
		p)
			patches_path="${OPTARG}"
			;;
		v)
			kernel_version="${OPTARG}"
			;;
		esac
	done

	shift $((OPTIND - 1))

	subcmd="${1:-}"

	[ -z "${subcmd}" ] && usage 1

	# If not kernel version take it from versions.yaml
	if [ -z "$kernel_version" ]; then
		kernel_version=$(get_from_kata_deps "assets.kernel.version")
		#Remove extra 'v'
		kernel_version="${kernel_version#v}"
	fi

	if [ -z "${kernel_path}" ]; then
		config_version=$(get_config_version)
		kernel_path="${PWD}/kata-linux-${kernel_version}-${config_version}"
	fi

	case "${subcmd}" in
	build)
		build_kernel "${kernel_path}"
		;;
	install)
		build_kernel "${kernel_path}"
		install_kata "${kernel_path}"
		;;
	setup)
		setup_kernel "${kernel_path}"
		[ -d "${kernel_path}" ] || die "${kernel_path} does not exist"
		echo "Kernel source ready: ${kernel_path} "
		;;
	*)
		usage 1
		;;

	esac
}

main $@
