#!/usr/bin/env bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

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
readonly kernel_config_repo="github.com/${project_name}/kata-containers/tools/packaging"
readonly patches_repo="github.com/${project_name}/kata-containers/tools/packaging"
# Default path to search patches to apply to kernel
readonly default_patches_dir="${script_dir}/patches"
# Default path to search config for kata
readonly default_kernel_config_dir="${script_dir}/configs"
# Default path to search for kernel config fragments
readonly default_config_frags_dir="${script_dir}/configs/fragments"
readonly default_config_whitelist="${script_dir}/configs/fragments/whitelist.conf"
# GPU vendor
readonly GV_INTEL="intel"
readonly GV_NVIDIA="nvidia"

#Path to kernel directory
kernel_path=""
#Experimental kernel support. Pull from virtio-fs GitLab instead of kernel.org
build_type=""
#Force generate config when setup
force_setup_generate_config="false"
#GPU kernel support
gpu_vendor=""
#Confidential guest type 
conf_guest=""
#
patches_path=""
#
hypervisor_target=""
#
arch_target=""
#
kernel_config_path=""
#
skip_config_checks="false"
# destdir
DESTDIR="${DESTDIR:-/}"
#PREFIX=
PREFIX="${PREFIX:-/usr}"
#Kernel URL
kernel_url=""

packaging_scripts_dir="${script_dir}/../scripts"
source "${packaging_scripts_dir}/lib.sh"

usage() {
	exit_code="$1"
	cat <<EOF
Overview:

	Build a kernel for Kata Containers

Usage:

	$script_name [options] <command> <argument>

Commands:

- setup

- build

- install

Options:

	-a <arch>   	: Arch target to build the kernel, such as aarch64/ppc64le/s390x/x86_64.
	-b <type>    	: Enable optional config type.
	-c <path>   	: Path to config file to build the kernel.
	-d          	: Enable bash debug.
	-e          	: Enable experimental kernel.
	-E          	: Enable arch-specific experimental kernel, arch info offered by "-a".
	-f          	: Enable force generate config when setup.
	-g <vendor> 	: GPU vendor, intel or nvidia.
	-h          	: Display this help.
	-k <path>   	: Path to kernel to build.
	-p <path>   	: Path to a directory with patches to apply to kernel.
	-s          	: Skip .config checks
	-t <hypervisor>	: Hypervisor_target.
	-u <url>	: Kernel URL to be used to download the kernel tarball.
	-v <version>	: Kernel version to use if kernel path not provided.
	-x <type>	: Confidential guest protection type, such as sev and tdx
EOF
	exit "$exit_code"
}

# Convert architecture to the name used by the Linux kernel build system
arch_to_kernel() {
	local -r arch="$1"

	case "$arch" in
		aarch64) echo "arm64" ;;
		ppc64le) echo "powerpc" ;;
		s390x) echo "s390" ;;
		x86_64) echo "$arch" ;;
		*) die "unsupported architecture: $arch" ;;
	esac
}

get_tee_kernel() {
	local version="${1}"
	local kernel_path="${2}"
	local tee="${3}"

	mkdir -p ${kernel_path}

	[ -z "${kernel_url}" ] && kernel_url=$(get_from_kata_deps "assets.kernel.${tee}.url")

	local kernel_tarball="${version}.tar.gz"

	# Depending on where we're getting the terball from it may have a
	# different name, such as linux-${version}.tar.gz or simply
	# ${version}.tar.gz.  Let's try both before failing.
	curl --fail -L "${kernel_url}/linux-${kernel_tarball}" -o ${kernel_tarball} || curl --fail -OL "${kernel_url}/${kernel_tarball}"
	
	mkdir -p ${kernel_path}
	tar --strip-components=1 -xf ${kernel_tarball} -C ${kernel_path}
}

get_kernel() {
	local version="${1:-}"

	local kernel_path=${2:-}
	[ -n "${kernel_path}" ] || die "kernel_path not provided"
	[ ! -d "${kernel_path}" ] || die "kernel_path already exist"

	if [ "${conf_guest}" != "" ]; then
		get_tee_kernel ${version} ${kernel_path} ${conf_guest}
		return
	fi

		#Remove extra 'v'
		version=${version#v}

		major_version=$(echo "${version}" | cut -d. -f1)
		kernel_tarball="linux-${version}.tar.xz"

                if [ ! -f sha256sums.asc ] || ! grep -q "${kernel_tarball}" sha256sums.asc; then
                        shasum_url="https://cdn.kernel.org/pub/linux/kernel/v${major_version}.x/sha256sums.asc"
                        info "Download kernel checksum file: sha256sums.asc from ${shasum_url}"
                        curl --fail -OL "${shasum_url}"
                fi
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

		tar xf "${kernel_tarball}"

		mv "linux-${version}" "${kernel_path}"
}

get_major_kernel_version() {
	local version="${1}"
	[ -n "${version}" ] || die "kernel version not provided"
	major_version=$(echo "${version}" | cut -d. -f1)
	minor_version=$(echo "${version}" | cut -d. -f2)
	echo "${major_version}.${minor_version}"
}

# Make a kernel config file from generic and arch specific
# fragments
# - arg1 - path to arch specific fragments
# - arg2 - path to kernel sources
#
get_kernel_frag_path() {
	local arch_path="$1"
	local common_path="${arch_path}/../common"
	local gpu_path="${arch_path}/../gpu"

	local kernel_path="$2"
	local arch="$3"
	local cmdpath="${kernel_path}/scripts/kconfig/merge_config.sh"
	local config_path="${arch_path}/.config"

	local arch_configs="$(ls ${arch_path}/*.conf)"
	# Exclude configs if they have !$arch tag in the header
	local common_configs="$(grep "\!${arch}" ${common_path}/*.conf -L)"

	local extra_configs=""
	if [ "${build_type}" != "" ];then
		local build_type_dir=$(readlink -m "${arch_path}/../build-type/${build_type}")
		if [ ! -d "$build_type_dir" ]; then
			die "No config fragments dir for ${build_type}: ${build_type_dir}"
		fi
		extra_configs=$(find "$build_type_dir" -name '*.conf')
		if [ "${extra_configs}" == "" ];then
			die "No extra configs found in ${build_type_dir}"
		fi
	fi

	# These are the strings that the kernel merge_config.sh script kicks out
	# when it reports an error or warning condition. We search for them in the
	# output to try and fail when we think something has been misconfigured.
	local not_in_string="not in final"
	local redefined_string="redefined"
	local redundant_string="redundant"

	# Later, if we need to add kernel version specific subdirs in order to
	# handle specific cases, then add the path definition and search/list/cat
	# here.
	local all_configs="${common_configs} ${arch_configs}"
	if [[ ${build_type} != "" ]]; then
		all_configs="${all_configs} ${extra_configs}"
	fi

	if [[ "${gpu_vendor}" != "" ]];then
		info "Add kernel config for GPU due to '-g ${gpu_vendor}'"
		local gpu_configs="$(ls ${gpu_path}/${gpu_vendor}.conf)"
		all_configs="${all_configs} ${gpu_configs}"
	fi

	if [[ "${conf_guest}" != "" ]];then
		info "Enabling config for '${conf_guest}' confidential guest protection"
		local conf_configs="$(ls ${arch_path}/${conf_guest}/*.conf)"
		all_configs="${all_configs} ${conf_configs}"
	fi

	info "Constructing config from fragments: ${config_path}"


	export KCONFIG_CONFIG=${config_path}
	export ARCH=${arch_target}
	cd ${kernel_path}

	local results
	results=$( ${cmdpath} -r -n ${all_configs} )
	# Only consider results highlighting "not in final"
	results=$(grep "${not_in_string}" <<< "$results")
	# Do not care about options that are in whitelist
	results=$(grep -v -f ${default_config_whitelist} <<< "$results")

	[[ "${skip_config_checks}" == "true" ]] && echo "${config_path}" && return

	# Did we request any entries that did not make it?
	local missing=$(echo $results | grep -v -q "${not_in_string}"; echo $?)
	if [ ${missing} -ne 0 ]; then
		info "Some CONFIG elements failed to make the final .config:"
		info "${results}"
		info "Generated config file can be found in ${config_path}"
		die "Failed to construct requested .config file"
	fi

	# Did we define something as two different values?
	local redefined=$(echo ${results} | grep -v -q "${redefined_string}"; echo $?)
	if [ ${redefined} -ne 0 ]; then
		info "Some CONFIG elements are redefined in fragments:"
		info "${results}"
		info "Generated config file can be found in ${config_path}"
		die "Failed to construct requested .config file"
	fi

	# Did we define something twice? Nominally this may not be an error, and it
	# might be convenient to allow it, but for now, let's pick up on them.
	local redundant=$(echo ${results} | grep -v -q "${redundant_string}"; echo $?)
	if [ ${redundant} -ne 0 ]; then
		info "Some CONFIG elements are redundant in fragments:"
		info "${results}"
		info "Generated config file can be found in ${config_path}"
		die "Failed to construct requested .config file"
	fi

	echo "${config_path}"
}

# Locate and return the path to the relevant kernel config file
# - arg1: kernel version
# - arg2: hypervisor target
# - arg3: arch target
# - arg4: kernel source path
get_default_kernel_config() {
	local version="${1}"

	local hypervisor="$2"
	local kernel_arch="$3"
	local kernel_path="$4"

	[ -n "${version}" ] || die "kernel version not provided"
	[ -n "${hypervisor}" ] || die "hypervisor not provided"
	[ -n "${kernel_arch}" ] || die "kernel arch not provided"

	local kernel_ver
	kernel_ver=$(get_major_kernel_version "${version}")

	archfragdir="${default_config_frags_dir}/${kernel_arch}"
	if [ -d "${archfragdir}" ]; then
		config="$(get_kernel_frag_path ${archfragdir} ${kernel_path} ${kernel_arch})"
	else
		[ "${hypervisor}" == "firecracker" ] && hypervisor="kvm"
		config="${default_kernel_config_dir}/${kernel_arch}_kata_${hypervisor}_${major_kernel}.x"
	fi

	[ -f "${config}" ] || die "failed to find default config ${config}"
	echo "${config}"
}

get_config_and_patches() {
	if [ -z "${patches_path}" ]; then
		patches_path="${default_patches_dir}"
	fi
}

get_config_version() {
	get_config_and_patches
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
		if [[ "${force_setup_generate_config}" != "true" ]];then
			return
		else
			info "Force generate config due to '-f'"
		fi
	else
		info "kernel path does not exist, will download kernel"
		download_kernel="true"
		[ -n "$kernel_version" ] || die "failed to get kernel version: Kernel version is emtpy"

		if [[ ${download_kernel} == "true" ]]; then
			get_kernel "${kernel_version}" "${kernel_path}"
		fi

		[ -n "$kernel_path" ] || die "failed to find kernel source path"
	fi

	get_config_and_patches

	[ -d "${patches_path}" ] || die " patches path '${patches_path}' does not exist"

	local major_kernel
	major_kernel=$(get_major_kernel_version "${kernel_version}")
	local patches_dir_for_version="${patches_path}/${major_kernel}.x"
	local build_type_patches_dir="${patches_path}/${major_kernel}.x/${build_type}"

	[ -n "${arch_target}" ] || arch_target="$(uname -m)"
	arch_target=$(arch_to_kernel "${arch_target}")
	(
	cd "${kernel_path}" || exit 1

	# Apply version specific patches
	${packaging_scripts_dir}/apply_patches.sh "${patches_dir_for_version}"

	# Apply version specific patches for build_type build
	if [ "${build_type}" != "" ] ;then
		info "Apply build_type patches from ${build_type_patches_dir}"
		${packaging_scripts_dir}/apply_patches.sh "${build_type_patches_dir}"
	fi

	[ -n "${hypervisor_target}" ] || hypervisor_target="kvm"
	[ -n "${kernel_config_path}" ] || kernel_config_path=$(get_default_kernel_config "${kernel_version}" "${hypervisor_target}" "${arch_target}" "${kernel_path}")

	info "Copying config file from: ${kernel_config_path}"
	cp "${kernel_config_path}" ./.config
	make oldconfig
	)
}

build_kernel() {
	local kernel_path=${1:-}
	[ -n "${kernel_path}" ] || die "kernel_path not provided"
	[ -d "${kernel_path}" ] || die "path to kernel does not exist, use ${script_name} setup"
	[ -n "${arch_target}" ] || arch_target="$(uname -m)"
	arch_target=$(arch_to_kernel "${arch_target}")
	pushd "${kernel_path}" >>/dev/null
	make -j $(nproc ${CI:+--ignore 1}) ARCH="${arch_target}"
	if [ "${conf_guest}" == "sev" ]; then
		make -j $(nproc ${CI:+--ignore 1}) INSTALL_MOD_STRIP=1 INSTALL_MOD_PATH=${kernel_path} modules_install
	fi
	[ "$arch_target" != "powerpc" ] && ([ -e "arch/${arch_target}/boot/bzImage" ] || [ -e "arch/${arch_target}/boot/Image.gz" ])
	[ -e "vmlinux" ]
	([ "${hypervisor_target}" == "firecracker" ] || [ "${hypervisor_target}" == "cloud-hypervisor" ]) && [ "${arch_target}" == "arm64" ] && [ -e "arch/${arch_target}/boot/Image" ]
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

	suffix=""
	if [[ ${build_type} != "" ]]; then
		suffix="-${build_type}"
	fi
	if [[ ${gpu_vendor} != "" ]];then
		suffix="-${gpu_vendor}-gpu${suffix}"
	fi

	if [[ ${conf_guest} != "" ]];then
		suffix="-${conf_guest}${suffix}"
	fi

	vmlinuz="vmlinuz-${kernel_version}-${config_version}${suffix}"
	vmlinux="vmlinux-${kernel_version}-${config_version}${suffix}"

	if [ -e "arch/${arch_target}/boot/bzImage" ]; then
		bzImage="arch/${arch_target}/boot/bzImage"
	elif [ -e "arch/${arch_target}/boot/Image.gz" ]; then
		bzImage="arch/${arch_target}/boot/Image.gz"
	elif [ "${arch_target}" != "powerpc" ]; then
		die "failed to find image"
	fi

	# Install compressed kernel
	if [ "${arch_target}" = "powerpc" ]; then
		install --mode 0644 -D "vmlinux" "${install_path}/${vmlinuz}"
	else
		install --mode 0644 -D "${bzImage}" "${install_path}/${vmlinuz}"
	fi

	# Install uncompressed kernel
	if [ "${arch_target}" = "arm64" ]; then
		install --mode 0644 -D "arch/${arch_target}/boot/Image" "${install_path}/${vmlinux}"
	elif [ "${arch_target}" = "s390" ]; then
		install --mode 0644 -D "arch/${arch_target}/boot/vmlinux" "${install_path}/${vmlinux}"
	else
		install --mode 0644 -D "vmlinux" "${install_path}/${vmlinux}"
	fi

	install --mode 0644 -D ./.config "${install_path}/config-${kernel_version}"

	ln -sf "${vmlinuz}" "${install_path}/vmlinuz${suffix}.container"
	ln -sf "${vmlinux}" "${install_path}/vmlinux${suffix}.container"
	ls -la "${install_path}/vmlinux${suffix}.container"
	ls -la "${install_path}/vmlinuz${suffix}.container"
	popd >>/dev/null
}

main() {
	while getopts "a:b:c:deEfg:hk:p:t:u:v:x:" opt; do	
		case "$opt" in
			a)
				arch_target="${OPTARG}"
				;;
			b)
				build_type="${OPTARG}"
				;;
			c)
				kernel_config_path="${OPTARG}"
				;;
			d)
				PS4=' Line ${LINENO}: '
				set -x
				;;
			e)
				build_type="experimental"
				;;
			E)
				build_type="arch-experimental"
				;;
			f)
				force_setup_generate_config="true"
				;;
			g)
				gpu_vendor="${OPTARG}"
				[[ "${gpu_vendor}" == "${GV_INTEL}" || "${gpu_vendor}" == "${GV_NVIDIA}" ]] || die "GPU vendor only support intel and nvidia"
				;;
			h)
				usage 0
				;;
			k)
				kernel_path="$(realpath ${OPTARG})"
				;;
			p)
				patches_path="${OPTARG}"
				;;
			s)
				skip_config_checks="true"
				;;
			t)
				hypervisor_target="${OPTARG}"
				;;
			u)	
				kernel_url="${OPTARG}"
				;;
			v)
				kernel_version="${OPTARG}"
				;;
			x)
				conf_guest="${OPTARG}"
				case "$conf_guest" in
					sev|tdx) ;;
					*) die "Confidential guest type '$conf_guest' not supported" ;;
				esac
				;;
		esac
	done

	shift $((OPTIND - 1))

	subcmd="${1:-}"

	[ -z "${subcmd}" ] && usage 1

	# If not kernel version take it from versions.yaml
	if [ -z "$kernel_version" ]; then
		if [[ ${build_type} == "experimental" ]]; then
			kernel_version=$(get_from_kata_deps "assets.kernel-experimental.tag")
		elif [[ ${build_type} == "arch-experimental" ]]; then
			case "${arch_target}" in
			"aarch64")
				build_type="arm-experimental"
				kernel_version=$(get_from_kata_deps "assets.arm-kernel-experimental.version")
			;;
			*)
				info "No arch-specific experimental kernel supported, using experimental one instead"
				kernel_version=$(get_from_kata_deps "assets.kernel-experimental.tag")
			;;
			esac
		elif [[ "${conf_guest}" != "" ]]; then
			#If specifying a tag for kernel_version, must be formatted version-like to avoid unintended parsing issues
			kernel_version=$(get_from_kata_deps "assets.kernel.${conf_guest}.version" 2>/dev/null || true)
			[ -n "${kernel_version}" ] || kernel_version=$(get_from_kata_deps "assets.kernel.${conf_guest}.tag")
		else
			kernel_version=$(get_from_kata_deps "assets.kernel.version")
		fi
	fi
	#Remove extra 'v'
	kernel_version="${kernel_version#v}"

	if [ -z "${kernel_path}" ]; then
		config_version=$(get_config_version)
		if [[ ${build_type} != "" ]]; then
			kernel_path="${PWD}/kata-linux-${build_type}-${kernel_version}-${config_version}"
		else
			kernel_path="${PWD}/kata-linux-${kernel_version}-${config_version}"
		fi
		info "Config version: ${config_version}"
	fi

	info "Kernel version: ${kernel_version}"

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
