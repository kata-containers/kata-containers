#!/usr/bin/env bash
#
# Copyright (c) 2024 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0

#!/bin/bash
set -xeuo pipefail

shopt -s nullglob
shopt -s extglob

run_file_name=$2
run_fm_file_name=$3
arch_target=$4
nvidia_gpu_stack="$5"
driver_version=""
driver_type="-open"
supported_gpu_devids="/supported-gpu.devids"
base_os="jammy"

APT_INSTALL="apt -o Dpkg::Options::='--force-confdef' -o Dpkg::Options::='--force-confold' -yqq --no-install-recommends install"

export DEBIAN_FRONTEND=noninteractive

is_feature_enabled() {
	local feature="$1"
	# Check if feature is in the comma-separated list
	if [[ ",$nvidia_gpu_stack," == *",$feature,"* ]]; then
		return 0
	else
		return 1
	fi
}

set_driver_version_type() {
	echo "chroot: Setting the correct driver version"

	if [[ ",$nvidia_gpu_stack," == *",latest,"* ]]; then
		driver_version="latest"
	elif [[ ",$nvidia_gpu_stack," == *",lts,"* ]]; then
		driver_version="lts"
	elif [[ "$nvidia_gpu_stack" =~ version=([^,]+) ]]; then
		driver_version="${BASH_REMATCH[1]}"
	else
		echo "No known driver spec found. Please specify \"latest\", \"lts\", or \"version=<VERSION>\"."
		exit 1
	fi

	echo "chroot: driver_version: ${driver_version}"

	echo "chroot: Setting the correct driver type"

	# driver       -> enable open or closed drivers
	if [[ "$nvidia_gpu_stack" =~ (^|,)driver=open($|,) ]]; then
		driver_type="-open"
	elif [[ "$nvidia_gpu_stack" =~ (^|,)driver=closed($|,) ]]; then
		driver_type=""
	fi

	echo "chroot: driver_type: ${driver_type}"
}

install_nvidia_ctk() {
	echo "chroot: Installing NVIDIA GPU container runtime"
	apt list nvidia-container-toolkit-base -a
	# Base  gives a nvidia-ctk and the nvidia-container-runtime
	eval "${APT_INSTALL}" nvidia-container-toolkit-base
}

install_nvidia_fabricmanager() {
	is_feature_enabled "nvswitch" || {
		echo "chroot: Skipping NVIDIA fabricmanager installation"
		return
	}
	# if run_fm_file_name exists run it
	if [ -f /"${run_fm_file_name}" ]; then
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

build_nvidia_drivers() {
	is_feature_enabled "compute" || {
		echo "chroot: Skipping NVIDIA drivers build"
		return
	}

	echo "chroot: Build NVIDIA drivers"
	pushd "${driver_source_files}" >> /dev/null

	local kernel_version
	for version in /lib/modules/*; do
		kernel_version=$(basename "${version}")
	        echo "chroot: Building GPU modules for: ${kernel_version}"
		cp /boot/System.map-"${kernel_version}" /lib/modules/"${kernel_version}"/build/System.map

		if [ "${arch_target}" == "aarch64" ]; then
			ln -sf /lib/modules/"${kernel_version}"/build/arch/arm64 /lib/modules/"${kernel_version}"/build/arch/aarch64
		fi

		if [ "${arch_target}" == "x86_64" ]; then
			ln -sf /lib/modules/"${kernel_version}"/build/arch/x86 /lib/modules/"${kernel_version}"/build/arch/amd64
		fi

		make -j "$(nproc)" CC=gcc SYSSRC=/lib/modules/"${kernel_version}"/build > /dev/null
		make INSTALL_MOD_STRIP=1 -j "$(nproc)" CC=gcc SYSSRC=/lib/modules/"${kernel_version}"/build modules_install
		make -j "$(nproc)" CC=gcc SYSSRC=/lib/modules/"${kernel_version}"/build clean > /dev/null

	done
	# Save the modules for later so that a linux-image purge does not remove it
	tar cvfa /lib/modules.save_from_purge.tar.zst /lib/modules
	popd >> /dev/null
}

install_userspace_components() {
	if [ ! -f /"${run_file_name}" ]; then
		echo "chroot: Skipping NVIDIA userspace runfile components installation"
		return
	fi

	pushd /NVIDIA-* >> /dev/null
	# if aarch64 we need to remove --no-install-compat32-libs
	if [ "${arch_target}" == "aarch64" ]; then
		./nvidia-installer --no-kernel-modules --no-systemd --no-nvidia-modprobe -s --x-prefix=/root
	else
		./nvidia-installer --no-kernel-modules --no-systemd --no-nvidia-modprobe -s --x-prefix=/root --no-install-compat32-libs
	fi
	popd >> /dev/null

}

prepare_run_file_drivers() {
	if [ "${driver_version}" == "latest" ]; then
		driver_version=""
		echo "chroot: Resetting driver version not supported with run-file"
	elif [ "${driver_version}" == "lts" ]; then
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
	if [ -e "${RIMFILE}" ]; then
		cp NVIDIA-*/RIM_GH100PROD.swidtag /usr/share/nvidia/rim/.
	fi
	popd >> /dev/null
}

prepare_distribution_drivers() {
	if [ "${driver_version}" == "latest" ]; then
		driver_version=$(apt-cache search --names-only 'nvidia-headless-no-dkms-.?.?.?-open' | awk '{ print $1 }' | tail -n 1 | cut -d'-' -f5)
	elif [ "${driver_version}" == "lts" ]; then
		driver_version="550"
	fi

	echo "chroot: Prepare NVIDIA distribution drivers"
	eval "${APT_INSTALL}" nvidia-headless-no-dkms-"${driver_version}${driver_type}" \
		libnvidia-cfg1-"${driver_version}"       \
		nvidia-compute-utils-"${driver_version}" \
		nvidia-utils-"${driver_version}"         \
		nvidia-kernel-common-"${driver_version}" \
		nvidia-imex-"${driver_version}"          \
		libnvidia-compute-"${driver_version}"    \
		libnvidia-compute-"${driver_version}"    \
		libnvidia-gl-"${driver_version}"         \
		libnvidia-extra-"${driver_version}"      \
		libnvidia-decode-"${driver_version}"     \
		libnvidia-fbc1-"${driver_version}"       \
		libnvidia-encode-"${driver_version}"
}

prepare_nvidia_drivers() {
	local driver_source_dir=""

	if [ -f /"${run_file_name}" ]; then
		prepare_run_file_drivers

		for source_dir in /NVIDIA-*; do
			if [ -d "${source_dir}" ]; then
				driver_source_files="${source_dir}"/kernel${driver_type}
				driver_source_dir="${source_dir}"
				break
			fi
		done
		get_supported_gpus_from_run_file "${driver_source_dir}"

	else
		prepare_distribution_drivers

		for source_dir in /usr/src/nvidia*; do
			if [ -d "${source_dir}" ]; then
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
	eval "${APT_INSTALL}" make gcc gawk kmod libvulkan1 pciutils jq zstd linuxptp
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

	# Changing the reference here also means changes needed for cuda_keyring
	# and cuda apt repository see install_dcgm for details
	cat <<-CHROOT_EOF > /etc/apt/sources.list.d/${base_os}.list
		deb [arch=amd64] http://us.archive.ubuntu.com/ubuntu ${base_os} main restricted universe multiverse
		deb [arch=amd64] http://us.archive.ubuntu.com/ubuntu ${base_os}-updates main restricted universe multiverse
		deb [arch=amd64] http://us.archive.ubuntu.com/ubuntu ${base_os}-security main restricted universe multiverse
		deb [arch=amd64] http://us.archive.ubuntu.com/ubuntu ${base_os}-backports main restricted universe multiverse

		deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports ${base_os} main restricted universe multiverse
		deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports ${base_os}-updates main restricted universe multiverse
		deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports ${base_os}-security main restricted universe multiverse
		deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports ${base_os}-backports main restricted universe multiverse
	CHROOT_EOF

	apt update

	eval "${APT_INSTALL}" curl gpg ca-certificates

	curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg
	curl -s -L https://nvidia.github.io/libnvidia-container/stable/deb/nvidia-container-toolkit.list |
		sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' |
		tee /etc/apt/sources.list.d/nvidia-container-toolkit.list

	apt update
}

install_kernel_dependencies() {
	dpkg -i /linux-*deb
}

get_supported_gpus_from_run_file() {
	local source_dir="$1"
	local supported_gpus_json="${source_dir}"/supported-gpus/supported-gpus.json

	jq . < "${supported_gpus_json}"  | grep '"devid"' | awk '{ print $2 }' | tr -d ',"'  > ${supported_gpu_devids}
}

get_supported_gpus_from_distro_drivers() {
	local supported_gpus_json=/usr/share/doc/nvidia-kernel-common-"${driver_version}"/supported-gpus.json

	jq . < "${supported_gpus_json}"  | grep '"devid"' | awk '{ print $2 }' | tr -d ',"'  > ${supported_gpu_devids}
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

	arch="x86_64"
	[[ ${arch_target} == "aarch64" ]] && arch="sbsa"
	# shellcheck disable=SC2015
	[[ ${base_os} == "jammy" ]] && osver="ubuntu2204" || die "Unknown base_os ${base_os} used"

	keyring="cuda-keyring_1.1-1_all.deb"
        curl -O https://developer.download.nvidia.com/compute/cuda/repos/${osver}/${arch}/${keyring}
        dpkg -i ${keyring} && rm -f ${keyring}

	apt update
	eval "${APT_INSTALL}" datacenter-gpu-manager
}

cleanup_rootfs() {
	echo "chroot: Cleanup NVIDIA GPU rootfs"

	apt-mark hold libstdc++6 libzstd1 libgnutls30 pciutils
	# noble=libgnutls30t64

	if [ -n "${driver_version}" ]; then
		apt-mark hold libnvidia-cfg1-"${driver_version}" \
			nvidia-compute-utils-"${driver_version}" \
			nvidia-utils-"${driver_version}"         \
			nvidia-kernel-common-"${driver_version}" \
			nvidia-imex-"${driver_version}"          \
			libnvidia-compute-"${driver_version}"    \
			libnvidia-compute-"${driver_version}"    \
			libnvidia-gl-"${driver_version}"         \
			libnvidia-extra-"${driver_version}"      \
			libnvidia-decode-"${driver_version}"     \
			libnvidia-fbc1-"${driver_version}"       \
			libnvidia-encode-"${driver_version}"	\
			libnvidia-nscq-"${driver_version}"	\
			linuxptp libnftnl11
	fi

	kernel_headers=$(dpkg --get-selections | cut -f1 | grep linux-headers)
	linux_images=$(dpkg --get-selections | cut -f1 | grep linux-image)
	for i in ${kernel_headers} ${linux_images}; do
		apt purge -yqq "${i}"
	done

	apt purge -yqq jq make gcc wget libc6-dev git xz-utils curl gpg \
		python3-pip software-properties-common ca-certificates  \
		linux-libc-dev nuitka python3-minimal

	if [ -n "${driver_version}" ]; then
		apt purge -yqq nvidia-headless-no-dkms-"${driver_version}${driver_type}" \
			nvidia-kernel-source-"${driver_version}${driver_type}" -yqq
	fi

	apt autoremove -yqq

	apt clean
	apt autoclean

	for modules_version in /lib/modules/*; do
		ln -sf "${modules_version}" /lib/modules/"$(uname -r)"
		touch  "${modules_version}"/modules.order
		touch  "${modules_version}"/modules.builtin
		depmod -a
	done

	rm -rf /etc/apt/sources.list* /var/lib/apt /var/log/apt /var/cache/debconf
	rm -f /usr/bin/nvidia-ngx-updater /usr/bin/nvidia-container-runtime
	rm -f /var/log/{nvidia-installer.log,dpkg.log,alternatives.log}

	# Clear and regenerate the ld cache
	rm -f /etc/ld.so.cache
	ldconfig

	tar xvf /lib/modules.save_from_purge.tar.zst -C /

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
cleanup_rootfs
