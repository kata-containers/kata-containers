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
# Default path to search patches to apply to kernel
readonly default_patches_dir="${script_dir}/patches"
# Default path to search config for kata
readonly default_kernel_config_dir="${script_dir}/configs"
# Default path to search for kernel config fragments
readonly default_config_frags_dir="${script_dir}/configs/fragments"
readonly default_config_whitelist="${script_dir}/configs/fragments/whitelist.conf"
readonly default_initramfs="${script_dir}/initramfs.cpio.gz"
# xPU vendor
readonly VENDOR_INTEL="intel"
readonly VENDOR_NVIDIA="nvidia"

#Path to kernel directory
kernel_path=""
#Experimental kernel support. Pull from virtio-fs GitLab instead of kernel.org
build_type=""
#Force generate config when setup
force_setup_generate_config="false"
#GPU kernel support
gpu_vendor=""
#DPU kernel support
dpu_vendor=""
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
#Linux headers for GPU guest fs module building
linux_headers=""
# Enable measurement of the guest rootfs at boot.
measured_rootfs="false"

CROSS_BUILD_ARG=""

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
	-D <vendor> 	: DPU/SmartNIC vendor, only nvidia.
	-d          	: Enable bash debug.
	-e          	: Enable experimental kernel.
	-E          	: Enable arch-specific experimental kernel, arch info offered by "-a".
	-f          	: Enable force generate config when setup, old kernel path and config will be removed.
	-g <vendor> 	: GPU vendor, intel or nvidia.
	-h          	: Display this help.
	-H <deb|rpm>	: Linux headers for guest fs module building.
	-m              : Enable measured rootfs.
	-k <path>   	: Path to kernel to build.
	-p <path>   	: Path to a directory with patches to apply to kernel.
	-s          	: Skip .config checks
	-t <hypervisor>	: Hypervisor_target.
	-u <url>	: Kernel URL to be used to download the kernel tarball.
	-v <version>	: Kernel version to use if kernel path not provided.
	-x       	: All the confidential guest protection type for a specific architecture.
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

# When building for measured rootfs the initramfs image should be previously built.
check_initramfs_or_die() {
	[ -f "${default_initramfs}" ] || \
		die "Initramfs for measured rootfs not found at ${default_initramfs}"
}

get_kernel() {
	local version="${1:-}"

	local kernel_path=${2:-}
	[ -n "${kernel_path}" ] || die "kernel_path not provided"
	[ ! -d "${kernel_path}" ] || die "kernel_path already exist"

	#Remove extra 'v'
	version=${version#v}

	local major_version=$(echo "${version}" | cut -d. -f1)
	local rc=$(echo "${version}" | grep -oE "\-rc[0-9]+$")

	local tar_suffix="tar.xz"
	if [ -n "${rc}" ]; then
		tar_suffix="tar.gz"
	fi
	kernel_tarball="linux-${version}.${tar_suffix}"

	if [ -z "${rc}" ]; then
		if [[ -f "${kernel_tarball}.sha256" ]] && (grep -qF "${kernel_tarball}" "${kernel_tarball}.sha256"); then
			info "Restore valid ${kernel_tarball}.sha256 to sha256sums.asc"
			cp -f "${kernel_tarball}.sha256" sha256sums.asc
		else
			shasum_url="https://cdn.kernel.org/pub/linux/kernel/v${major_version}.x/sha256sums.asc"
			info "Download kernel checksum file: sha256sums.asc from ${shasum_url}"
			curl --fail -OL "${shasum_url}"
			if (grep -F "${kernel_tarball}" sha256sums.asc >"${kernel_tarball}.sha256"); then
				info "sha256sums.asc is valid, ${kernel_tarball}.sha256 generated"
			else
				die "sha256sums.asc is invalid"
			fi
		fi
	else
		info "Release candidate kernels are not part of the official sha256sums.asc -- skipping sha256sum validation"
	fi

	if [ -f "${kernel_tarball}" ]; then
	       	if [ -n "${rc}" ] && ! sha256sum -c "${kernel_tarball}.sha256"; then
			info "invalid kernel tarball ${kernel_tarball} removing "
			rm -f "${kernel_tarball}"
		fi
	fi
	if [ ! -f "${kernel_tarball}" ]; then
		kernel_tarball_url="https://www.kernel.org/pub/linux/kernel/v${major_version}.x/${kernel_tarball}"
		if [ -n "${kernel_url}" ]; then
			kernel_tarball_url="${kernel_url}${kernel_tarball}"
		fi
		info "Download kernel version ${version}"
		info "Download kernel from: ${kernel_tarball_url}"
		curl --fail -OL "${kernel_tarball_url}"
	else
		info "kernel tarball already downloaded"
	fi

	if [ -z "${rc}" ]; then
		sha256sum -c "${kernel_tarball}.sha256"
	fi

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
	local dpu_path="${arch_path}/../dpu"

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
		# If conf_guest is set we need to update the CONFIG_LOCALVERSION
		# to match the suffix created in install_kata
		# -nvidia-gpu-confidential, the linux headers will be named the very
		# same if build with make deb-pkg for TDX or SNP.
		local gpu_configs=$(mktemp).conf
		local gpu_subst_configs="${gpu_path}/${gpu_vendor}.${arch_target}.conf.in"
		if [[ "${conf_guest}" != "" ]];then
			export CONF_GUEST_SUFFIX="-${conf_guest}"
		else
			export CONF_GUEST_SUFFIX=""
		fi
		envsubst <${gpu_subst_configs} >${gpu_configs}
		unset CONF_GUEST_SUFFIX

		all_configs="${all_configs} ${gpu_configs}"
	fi

	if [[ "${dpu_vendor}" != "" ]]; then
		info "Add kernel config for DPU/SmartNIC due to '-n ${dpu_vendor}'"
		local dpu_configs="${dpu_path}/${dpu_vendor}.conf"
		all_configs="${all_configs} ${dpu_configs}"
	fi

	if [ "${measured_rootfs}" == "true" ]; then
		info "Enabling config for confidential guest trust storage protection"
		local cryptsetup_configs="$(ls ${common_path}/confidential_containers/cryptsetup.conf)"
		all_configs="${all_configs} ${cryptsetup_configs}"

		check_initramfs_or_die
		info "Enabling config for confidential guest measured boot"
		local initramfs_configs="$(ls ${common_path}/confidential_containers/initramfs.conf)"
		all_configs="${all_configs} ${initramfs_configs}"
	fi

	if [[ "${conf_guest}" != "" ]];then
		info "Enabling config for '${conf_guest}' confidential guest protection"
		local conf_configs="$(ls ${arch_path}/${conf_guest}/*.conf)"
		all_configs="${all_configs} ${conf_configs}"

		local tmpfs_configs="$(ls ${common_path}/confidential_containers/tmpfs.conf)"
		all_configs="${all_configs} ${tmpfs_configs}"
	fi

	if [[ "$force_setup_generate_config" == "true" ]]; then
		info "Remove existing config ${config_path} due to '-f'"
		[ -f "$config_path" ] && rm -f "${config_path}"
		[ -f "$config_path".old ] && rm -f "${config_path}".old
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

	if [[ "$force_setup_generate_config" == "true" ]] && [[ -d "$kernel_path" ]];then
		info "Remove existing directory ${kernel_path} due to '-f'"
		rm -rf "${kernel_path}"
	fi

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

	if [ "${measured_rootfs}" == "true" ]; then
		check_initramfs_or_die
		info "Copying initramfs from: ${default_initramfs}"
		cp "${default_initramfs}" ./
	fi

	info "Copying config file from: ${kernel_config_path}"
	cp "${kernel_config_path}" ./.config
	ARCH=${arch_target}  make oldconfig ${CROSS_BUILD_ARG}
	)
}

build_kernel() {
	local kernel_path=${1:-}
	[ -n "${kernel_path}" ] || die "kernel_path not provided"
	[ -d "${kernel_path}" ] || die "path to kernel does not exist, use ${script_name} setup"
	[ -n "${arch_target}" ] || arch_target="$(uname -m)"
	arch_target=$(arch_to_kernel "${arch_target}")
	pushd "${kernel_path}" >>/dev/null
	make -j $(nproc) ARCH="${arch_target}" ${CROSS_BUILD_ARG}
	if [ "${conf_guest}" == "confidential" ]; then
		make -j $(nproc) INSTALL_MOD_STRIP=1 INSTALL_MOD_PATH=${kernel_path} modules_install
	fi
	[ "$arch_target" != "powerpc" ] && ([ -e "arch/${arch_target}/boot/bzImage" ] || [ -e "arch/${arch_target}/boot/Image.gz" ])
	[ -e "vmlinux" ]
	([ "${hypervisor_target}" == "firecracker" ] || [ "${hypervisor_target}" == "cloud-hypervisor" ]) && [ "${arch_target}" == "arm64" ] && [ -e "arch/${arch_target}/boot/Image" ]
	popd >>/dev/null
}

build_kernel_headers() {
	local kernel_path=${1:-}
	[ -n "${kernel_path}" ] || die "kernel_path not provided"
	[ -d "${kernel_path}" ] || die "path to kernel does not exist, use ${script_name} setup"
	[ -n "${arch_target}" ] || arch_target="$(uname -m)"
	arch_target=$(arch_to_kernel "${arch_target}")
	pushd "${kernel_path}" >>/dev/null

	if [ "$linux_headers" == "deb" ]; then
		export KBUILD_BUILD_USER="${USER}"
		make -j $(nproc) bindeb-pkg ARCH="${arch_target}"
	fi
	if [ "$linux_headers" == "rpm" ]; then
		make -j $(nproc) rpm-pkg ARCH="${arch_target}"
	fi

	popd >>/dev/null
}

install_kata() {
	local kernel_path=${1:-}
	[ -n "${kernel_path}" ] || die "kernel_path not provided"
	[ -d "${kernel_path}" ] || die "path to kernel does not exist, use ${script_name} setup"
	[ -n "${arch_target}" ] || arch_target="$(uname -m)"
	arch_target=$(arch_to_kernel "${arch_target}")
	pushd "${kernel_path}" >>/dev/null
	config_version=$(get_config_version)
	[ -n "${config_version}" ] || die "failed to get config version"
	install_path=$(readlink -m "${DESTDIR}/${PREFIX}/share/${project_name}")

	suffix=""
	if [[ ${build_type} != "" ]]; then
		suffix="-${build_type}"
	fi

	if [[ ${conf_guest} != "" ]];then
		suffix="-${conf_guest}${suffix}"
	fi

	if [[ ${gpu_vendor} != "" ]];then
		suffix="-${gpu_vendor}-gpu${suffix}"
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

	install --mode 0644 -D ./.config "${install_path}/config-${kernel_version}-${config_version}${suffix}"

	ln -sf "${vmlinuz}" "${install_path}/vmlinuz${suffix}.container"
	ln -sf "${vmlinux}" "${install_path}/vmlinux${suffix}.container"
	ls -la "${install_path}/vmlinux${suffix}.container"
	ls -la "${install_path}/vmlinuz${suffix}.container"
	popd >>/dev/null
}

main() {
	while getopts "a:b:c:dD:eEfg:hH:k:mp:st:u:v:x" opt; do
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
			D)
				dpu_vendor="${OPTARG}"
				[[ "${dpu_vendor}" == "${VENDOR_NVIDIA}" ]] || die "DPU vendor only support nvidia"
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
				[[ "${gpu_vendor}" == "${VENDOR_INTEL}" || "${gpu_vendor}" == "${VENDOR_NVIDIA}" ]] || die "GPU vendor only support intel and nvidia"
				;;
			h)
				usage 0
				;;
			H)
				linux_headers="${OPTARG}"
				;;
			m)
				measured_rootfs="true"
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
				conf_guest="confidential"
				;;
			*)
				echo "ERROR: invalid argument '$opt'"
				exit 1
				;;
		esac
	done

	shift $((OPTIND - 1))

	subcmd="${1:-}"

	[ -z "${subcmd}" ] && usage 1

	if [[ ${build_type} == "experimental" ]] && [[ ${hypervisor_target} == "dragonball" ]]; then
		build_type="dragonball-experimental"
		if [ -n "$kernel_version" ];  then
			kernel_major_version=$(get_major_kernel_version "${kernel_version}")
			if [[ ${kernel_major_version} != "5.10" ]]; then
				info "dragonball-experimental kernel patches are only tested on 5.10.x kernel now, other kernel version may cause confliction"
			fi
		fi
	fi

	# If not kernel version take it from versions.yaml
	if [ -z "$kernel_version" ]; then
		if [[ ${build_type} == "experimental" ]]; then
			kernel_version=$(get_from_kata_deps ".assets.kernel-experimental.tag")
		elif [[ ${build_type} == "arch-experimental" ]]; then
			case "${arch_target}" in
			"aarch64")
				build_type="arm-experimental"
				kernel_version=$(get_from_kata_deps ".assets.kernel-arm-experimental.version")
			;;
			*)
				info "No arch-specific experimental kernel supported, using experimental one instead"
				kernel_version=$(get_from_kata_deps ".assets.kernel-experimental.tag")
			;;
			esac
		elif [[ ${build_type} == "dragonball-experimental" ]]; then
			kernel_version=$(get_from_kata_deps ".assets.kernel-dragonball-experimental.version")
		elif [[ "${conf_guest}" != "" ]]; then
			#If specifying a tag for kernel_version, must be formatted version-like to avoid unintended parsing issues
			kernel_version=$(get_from_kata_deps ".assets.kernel.${conf_guest}.version" 2>/dev/null || true)
			[ -n "${kernel_version}" ] || kernel_version=$(get_from_kata_deps ".assets.kernel.${conf_guest}.tag")
		else
			kernel_version=$(get_from_kata_deps ".assets.kernel.version")
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

	[ "${arch_target}" != "" -a "${arch_target}" != $(uname -m) ] && CROSS_BUILD_ARG="CROSS_COMPILE=${arch_target}-linux-gnu-"

	case "${subcmd}" in
		build)
			build_kernel "${kernel_path}"
			;;
		build-headers)
			build_kernel_headers "${kernel_path}"
			;;
		install)
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
