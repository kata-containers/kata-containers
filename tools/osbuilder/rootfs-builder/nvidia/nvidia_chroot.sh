#!/usr/bin/env bash
#
# Copyright (c) 2024 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0

#!/bin/bash
set -euo pipefail
[[ -n "${DEBUG}" ]] && set -x

shopt -s nullglob
shopt -s extglob

# Error helpers
trap 'echo "chroot: ERROR at line ${LINENO}: ${BASH_COMMAND}" >&2' ERR
die() {
  local msg="${*:-fatal error}"
  echo "chroot: ${msg}" >&2
  exit 1
}

run_file_name=$2
run_fm_file_name=$3
arch_target=$4
nvidia_gpu_stack="$5"
driver_version=""
driver_type="-open"
supported_gpu_devids="/supported-gpu.devids"
base_os="noble"

APT_INSTALL="apt -o Dpkg::Options::='--force-confdef' -o Dpkg::Options::='--force-confold' -yqq --no-install-recommends install"

export KBUILD_SIGN_PIN="${6:-}"

export DEBIAN_FRONTEND=noninteractive

is_feature_enabled() {
	local feature="$1"
	# Check if feature is in the comma-separated list
	if [[ ",${nvidia_gpu_stack}," == *",${feature},"* ]]; then
		return 0
	else
		return 1
	fi
}

set_driver_version_type() {
	echo "chroot: Setting the correct driver version"

	if [[ ",${nvidia_gpu_stack}," == *",latest,"* ]]; then
		driver_version="latest"
	elif [[ ",${nvidia_gpu_stack}," == *",lts,"* ]]; then
		driver_version="lts"
	elif [[ "${nvidia_gpu_stack}" =~ version=([^,]+) ]]; then
		driver_version="${BASH_REMATCH[1]}"
	else
		echo "No known driver spec found. Please specify \"latest\", \"lts\", or \"version=<VERSION>\"."
		exit 1
	fi

	echo "chroot: driver_version: ${driver_version}"

	echo "chroot: Setting the correct driver type"

	# driver       -> enable open or closed drivers
	if [[ "${nvidia_gpu_stack}" =~ (^|,)driver=open($|,) ]]; then
		driver_type="-open"
	elif [[ "${nvidia_gpu_stack}" =~ (^|,)driver=closed($|,) ]]; then
		driver_type=""
	fi

	echo "chroot: driver_type: ${driver_type}"
}

install_nvidia_ctk() {
	echo "chroot: Installing NVIDIA GPU container runtime"
	apt list nvidia-container-toolkit-base -a
	# Base  gives a nvidia-ctk and the nvidia-container-runtime
	eval "${APT_INSTALL}" nvidia-container-toolkit-base=1.17.6-1
}

install_nvidia_fabricmanager() {
	is_feature_enabled "nvswitch" || {
		echo "chroot: Skipping NVIDIA fabricmanager installation"
		return
	}
	# if run_fm_file_name exists run it
	if [[ -f /"${run_fm_file_name}" ]]; then
		install_nvidia_fabricmanager_from_run_file
	else
		install_nvidia_fabricmanager_from_distribution
	fi
}

install_nvidia_fabricmanager_from_run_file() {
	echo "chroot: Install NVIDIA fabricmanager from run file"
	pushd / >> /dev/null
	chmod +x "${run_fm_file_name}"
	./"${run_fm_file_name}" --nox11
	popd >> /dev/null
}

install_nvidia_fabricmanager_from_distribution() {
	echo "chroot: Install NVIDIA fabricmanager from distribution"
	eval "${APT_INSTALL}" nvidia-fabricmanager-"${driver_version}" libnvidia-nscq-"${driver_version}"
	apt-mark hold nvidia-fabricmanager-"${driver_version}"  libnvidia-nscq-"${driver_version}"
}

check_kernel_sig_config() {
	[[ -n ${kernel_version} ]] || die "kernel_version is not set"
	[[ -e /lib/modules/"${kernel_version}"/build/scripts/config ]] || die  "Cannot find /lib/modules/${kernel_version}/build/scripts/config"
	# make sure the used kernel has the proper CONFIG(s) set
	readonly scripts_config=/lib/modules/"${kernel_version}"/build/scripts/config
	[[ "$("${scripts_config}" --file "/boot/config-${kernel_version}" --state CONFIG_MODULE_SIG)" == "y" ]] || die  "Kernel config CONFIG_MODULE_SIG must be =Y"
	[[ "$("${scripts_config}" --file "/boot/config-${kernel_version}" --state CONFIG_MODULE_SIG_FORCE)" == "y" ]] || die  "Kernel config CONFIG_MODULE_SIG_FORCE must be =Y"
	[[ "$("${scripts_config}" --file "/boot/config-${kernel_version}" --state CONFIG_MODULE_SIG_ALL)" == "y" ]] || die  "Kernel config CONFIG_MODULE_SIG_ALL must be =Y"
	[[ "$("${scripts_config}" --file "/boot/config-${kernel_version}" --state CONFIG_MODULE_SIG_SHA512)" == "y" ]] || die  "Kernel config CONFIG_MODULE_SIG_SHA512 must be =Y"
	[[ "$("${scripts_config}" --file "/boot/config-${kernel_version}" --state CONFIG_SYSTEM_TRUSTED_KEYS)" == "" ]] || die  "Kernel config CONFIG_SYSTEM_TRUSTED_KEYS must be =\"\""
	[[ "$("${scripts_config}" --file "/boot/config-${kernel_version}" --state CONFIG_SYSTEM_TRUSTED_KEYRING)" == "y" ]] || die  "Kernel config CONFIG_SYSTEM_TRUSTED_KEYRING must be =Y"
}

build_nvidia_drivers() {
	is_feature_enabled "compute" || {
		echo "chroot: Skipping NVIDIA drivers build"
		return
	}

	echo "chroot: Build NVIDIA drivers"
	pushd "${driver_source_files}" >> /dev/null

	local certs_dir
	local kernel_version
	local ARCH
	for version in /lib/modules/*; do
		kernel_version=$(basename "${version}")
		certs_dir=/lib/modules/"${kernel_version}"/build/certs
		signing_key=${certs_dir}/signing_key.pem

	        echo "chroot: Building GPU modules for: ${kernel_version}"
		cp /boot/System.map-"${kernel_version}" /lib/modules/"${kernel_version}"/build/System.map

		if [[ "${arch_target}" == "aarch64" ]]; then
			ln -sf /lib/modules/"${kernel_version}"/build/arch/arm64 /lib/modules/"${kernel_version}"/build/arch/aarch64
			ARCH=arm64
		fi

		if [[ "${arch_target}" == "x86_64" ]]; then
			ln -sf /lib/modules/"${kernel_version}"/build/arch/x86 /lib/modules/"${kernel_version}"/build/arch/amd64
			ARCH=x86_64
		fi

		echo "chroot: Building GPU modules for: ${kernel_version} ${ARCH}"

		make -j "$(nproc)" CC=gcc SYSSRC=/lib/modules/"${kernel_version}"/build > /dev/null

		if [[ -n "${KBUILD_SIGN_PIN}" ]]; then
			mkdir -p "${certs_dir}" && mv /signing_key.* "${certs_dir}"/.
			check_kernel_sig_config
		fi

		make INSTALL_MOD_STRIP=1 -j "$(nproc)" CC=gcc SYSSRC=/lib/modules/"${kernel_version}"/build modules_install
		make -j "$(nproc)" CC=gcc SYSSRC=/lib/modules/"${kernel_version}"/build clean > /dev/null
		# The make clean above should clear also the certs directory but just in case something
		# went wroing make sure the signing_key.pem is removed
		[[ -e "${signing_key}" ]] && rm -f "${signing_key}"
	done
	popd >> /dev/null
}

install_userspace_components() {
	if [[ ! -f /"${run_file_name}" ]]; then
		echo "chroot: Skipping NVIDIA userspace runfile components installation"
		return
	fi

	pushd /NVIDIA-* >> /dev/null
	# if aarch64 we need to remove --no-install-compat32-libs
	if [[ "${arch_target}" == "aarch64" ]]; then
		./nvidia-installer --no-kernel-modules --no-systemd --no-nvidia-modprobe -s --x-prefix=/root
	else
		./nvidia-installer --no-kernel-modules --no-systemd --no-nvidia-modprobe -s --x-prefix=/root --no-install-compat32-libs
	fi
	popd >> /dev/null

}

prepare_run_file_drivers() {
	if [[ "${driver_version}" == "latest" ]]; then
		driver_version=""
		echo "chroot: Resetting driver version not supported with run-file"
	elif [[ "${driver_version}" == "lts" ]]; then
		driver_version=""
		echo "chroot: Resetting driver version not supported with run-file"
	fi

	echo "chroot: Prepare NVIDIA run file drivers"
	pushd / >> /dev/null
	chmod +x "${run_file_name}"
	./"${run_file_name}" -x

	mkdir -p /usr/share/nvidia/rim/

	# Sooner or later RIM files will be only available remotely
	RIMFILE=$(ls NVIDIA-*/RIM_GH100PROD.swidtag)
	if [[ -e "${RIMFILE}" ]]; then
		cp NVIDIA-*/RIM_GH100PROD.swidtag /usr/share/nvidia/rim/.
	fi
	popd >> /dev/null
}

prepare_distribution_drivers() {
	if [[ "${driver_version}" == "latest" ]]; then
		driver_version=$(apt-cache search --names-only 'nvidia-headless-no-dkms-.?.?.?-open' | sort | awk '{ print $1 }' | tail -n 1 | cut -d'-' -f5)
	elif [[ "${driver_version}" == "lts" ]]; then
		driver_version="550"
	fi

	echo "chroot: Prepare NVIDIA distribution drivers"

	eval "${APT_INSTALL}" nvidia-utils-"${driver_version}"

	eval "${APT_INSTALL}" nvidia-headless-no-dkms-"${driver_version}${driver_type}" \
		nvidia-firmware-"${driver_version}"  \
		nvidia-imex-"${driver_version}"      \
		libnvidia-cfg1-"${driver_version}"   \
		libnvidia-gl-"${driver_version}"     \
		libnvidia-extra-"${driver_version}"  \
		libnvidia-decode-"${driver_version}" \
		libnvidia-fbc1-"${driver_version}"   \
		libnvidia-encode-"${driver_version}" \
		libnvidia-nscq-"${driver_version}"
}

prepare_nvidia_drivers() {
	local driver_source_dir=""

	if [[ -f /"${run_file_name}" ]]; then
		prepare_run_file_drivers

		for source_dir in /NVIDIA-*; do
			if [[ -d "${source_dir}" ]]; then
				driver_source_files="${source_dir}"/kernel${driver_type}
				driver_source_dir="${source_dir}"
				break
			fi
		done
		get_supported_gpus_from_run_file "${driver_source_dir}"

	else
		prepare_distribution_drivers

		for source_dir in /usr/src/nvidia*; do
			if [[ -d "${source_dir}" ]]; then
				driver_source_files="${source_dir}"
				driver_source_dir="${source_dir}"
				break
			fi
		done
		get_supported_gpus_from_distro_drivers "${driver_source_dir}"
	fi

}

install_build_dependencies() {
	echo "chroot: Install NVIDIA drivers build dependencies"
	eval "${APT_INSTALL}" make gcc gawk kmod libvulkan1 pciutils jq zstd linuxptp xz-utils
}

setup_apt_repositories() {
	echo "chroot: Setup APT repositories"
	mkdir -p /var/cache/apt/archives/partial
	mkdir -p /var/log/apt
        mkdir -p /var/lib/dpkg/info
        mkdir -p /var/lib/dpkg/updates
        mkdir -p /var/lib/dpkg/alternatives
        mkdir -p /var/lib/dpkg/triggers
        mkdir -p /var/lib/dpkg/parts
	touch /var/lib/dpkg/status
	rm -f /etc/apt/sources.list.d/*

	if [[ "${arch_target}" == "x86_64" ]]; then
		cat <<-CHROOT_EOF > /etc/apt/sources.list.d/"${base_os}".list
			deb [arch=amd64] http://us.archive.ubuntu.com/ubuntu ${base_os} main restricted universe multiverse
			deb [arch=amd64] http://us.archive.ubuntu.com/ubuntu ${base_os}-updates main restricted universe multiverse
			deb [arch=amd64] http://us.archive.ubuntu.com/ubuntu ${base_os}-security main restricted universe multiverse
			deb [arch=amd64] http://us.archive.ubuntu.com/ubuntu ${base_os}-backports main restricted universe multiverse
		CHROOT_EOF
	fi

	if [[ "${arch_target}" == "aarch64" ]]; then
		cat <<-CHROOT_EOF > /etc/apt/sources.list.d/"${base_os}".list
			deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports ${base_os} main restricted universe multiverse
			deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports ${base_os}-updates main restricted universe multiverse
			deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports ${base_os}-security main restricted universe multiverse
			deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports ${base_os}-backports main restricted universe multiverse
		CHROOT_EOF
	fi

	local arch="${arch_target}"
	[[ ${arch_target} == "aarch64" ]] && arch="sbsa"
	# shellcheck disable=SC2015
	[[ ${base_os} == "noble" ]] && osver="ubuntu2404" || die "Unknown base_os ${base_os} used"

	keyring="cuda-keyring_1.1-1_all.deb"
    curl -O "https://developer.download.nvidia.com/compute/cuda/repos/${osver}/${arch}/${keyring}"
    dpkg -i "${keyring}" && rm -f "${keyring}"

	# Set repository priorities, prefere NVIDIA repositories over Ubuntu ones
	cat <<-CHROOT_EOF > /etc/apt/preferences.d/nvidia-priority
        # Prioritize NVIDIA CUDA repository
        Package: *
        Pin: origin developer.download.nvidia.com
        Pin-Priority: 1000

        # Prioritize NVIDIA Container Toolkit repository
        Package: *
        Pin: origin nvidia.github.io
        Pin-Priority: 950

        # Lower priority for Ubuntu repositories
        Package: *
        Pin: origin us.archive.ubuntu.com
        Pin-Priority: 500

        Package: *
        Pin: origin ports.ubuntu.com
        Pin-Priority: 500
	CHROOT_EOF

	apt update
}

install_kernel_dependencies() {
	dpkg -i /linux-*deb
}

get_supported_gpus_from_run_file() {
	local source_dir="$1"
	local supported_gpus_json="${source_dir}"/supported-gpus/supported-gpus.json

	jq . < "${supported_gpus_json}"  | grep '"devid"' | awk '{ print $2 }' | tr -d ',"'  > "${supported_gpu_devids}"
}

get_supported_gpus_from_distro_drivers() {
	local supported_gpus_json=./usr/share/doc/nvidia-driver-"${driver_version}"/supported-gpus.json

	mkdir _tmp
	pushd _tmp >> /dev/null

	apt download nvidia-driver-"${driver_version}"
	ar -x nvidia-driver-"${driver_version}"*.deb
	tar -xvf data.tar.xz

	jq . < "${supported_gpus_json}"  | grep '"devid"' | awk '{ print $2 }' | tr -d ',"'  > "${supported_gpu_devids}"

	popd >> /dev/null
	rm -rf _tmp
}

export_driver_version() {
	for modules_version in /lib/modules/*; do
        	modinfo "${modules_version}"/kernel/drivers/video/nvidia.ko | grep ^version | awk '{ print $2 }' > /nvidia_driver_version
		break
	done
}

install_nvidia_dcgm() {
	is_feature_enabled "dcgm" || {
		echo "chroot: Skipping NVIDIA DCGM installation"
		return
	}

	echo "chroot: Install NVIDIA DCGM"

	eval "${APT_INSTALL}" datacenter-gpu-manager \
		datacenter-gpu-manager-exporter
}

# Start of script
echo "chroot: Setup NVIDIA GPU rootfs stage one"

set_driver_version_type
setup_apt_repositories
install_kernel_dependencies
install_build_dependencies
prepare_nvidia_drivers
build_nvidia_drivers
install_userspace_components
install_nvidia_fabricmanager
install_nvidia_ctk
export_driver_version
install_nvidia_dcgm
