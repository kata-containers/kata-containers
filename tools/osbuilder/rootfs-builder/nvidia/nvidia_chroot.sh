#!/usr/bin/env bash
#
# Copyright (c) 2024 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0

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
base_os="noble"

APT_INSTALL="apt -o Dpkg::Options::='--force-confdef' -o Dpkg::Options::='--force-confold' -yqq --no-install-recommends install"

export DEBIAN_FRONTEND=noninteractive

is_feature_enabled() {
	local feature="$1"
	[[ ",${nvidia_gpu_stack}," == *",${feature},"* ]]
}

set_driver_version() {
	# Extract the driver=XXX part first, then get the value
	if [[ "${nvidia_gpu_stack}" =~ driver=([^,]+) ]]; then
		driver_version="${BASH_REMATCH[1]}"
	fi
	echo "chroot: driver_version: ${driver_version}"
	echo "chroot:  TODO remove with new NVRC"
	cat <<-CHROOT_EOF > "/supported-gpu.devids"
		0x230E
		0x2321
		0x2322
		0x2324
		0x2329
		0x232C
		0x2330
		0x2331
		0x2335
		0x2339
		0x233A
		0x233B
		0x2342
		0x2348
	CHROOT_EOF
}

install_nvidia_ctk() {
	echo "chroot: Installing NVIDIA GPU container runtime"
	# Base  gives a nvidia-ctk and the nvidia-container-runtime
	eval "${APT_INSTALL}" nvidia-container-toolkit-base=1.17.6-1
}

install_nvidia_fabricmanager() {
	is_feature_enabled "nvswitch" || {
		echo "chroot: Skipping NVIDIA fabricmanager installation"
		return
	}
	echo "chroot: Install NVIDIA fabricmanager"
	eval "${APT_INSTALL}" nvidia-fabricmanager libnvidia-nscq
	apt-mark hold nvidia-fabricmanager libnvidia-nscq
}

install_userspace_components() {
	eval "${APT_INSTALL}" nvidia-driver-pinning-"${driver_version}"
	eval "${APT_INSTALL}" nvidia-imex nvidia-firmware        \
		libnvidia-cfg1 libnvidia-gl libnvidia-extra      \
		libnvidia-decode libnvidia-fbc1 libnvidia-encode \
		libnvidia-nscq

	apt-mark hold nvidia-imex nvidia-firmware                \
		libnvidia-cfg1 libnvidia-gl libnvidia-extra      \
		libnvidia-decode libnvidia-fbc1 libnvidia-encode \
		libnvidia-nscq
}

setup_apt_repositories() {
	echo "chroot: Setup APT repositories"

	# Architecture to mirror mapping
	declare -A arch_to_mirror=(
		["x86_64"]="us.archive.ubuntu.com/ubuntu"
		["aarch64"]="ports.ubuntu.com/ubuntu-ports"
	)

	local mirror="${arch_to_mirror[${arch_target}]}"
	[[ -z "${mirror}" ]] && die "Unknown arch_target: ${arch_target}"

	local deb_arch="amd64"
	[[ "${arch_target}" == "aarch64" ]] && deb_arch="arm64"

	mkdir -p /var/cache/apt/archives/partial /var/log/apt                  \
		/var/lib/dpkg/{info,updates,alternatives,triggers,parts}

	touch /var/lib/dpkg/status

	rm -f /etc/apt/sources.list.d/*

	key="/usr/share/keyrings/ubuntu-archive-keyring.gpg"

	cat <<-CHROOT_EOF > /etc/apt/sources.list.d/"${base_os}".list
		deb [arch=${deb_arch} signed-by=${key}] http://${mirror} ${base_os} main restricted universe multiverse
		deb [arch=${deb_arch} signed-by=${key}] http://${mirror} ${base_os}-updates main restricted universe multiverse
		deb [arch=${deb_arch} signed-by=${key}] http://${mirror} ${base_os}-security main restricted universe multiverse
		deb [arch=${deb_arch} signed-by=${key}] http://${mirror} ${base_os}-backports main restricted universe multiverse
	CHROOT_EOF

	local arch="${arch_target}"
	[[ ${arch_target} == "aarch64" ]] && arch="sbsa"
	# shellcheck disable=SC2015
	[[ ${base_os} == "noble" ]] && osver="ubuntu2404" || die "Unknown base_os ${base_os} used"

	keyring="cuda-keyring_1.1-1_all.deb"
	# Use consistent curl flags: -fsSL for download, -O for output
	curl -fsSL -O "https://developer.download.nvidia.com/compute/cuda/repos/${osver}/${arch}/${keyring}"
	dpkg -i "${keyring}" && rm -f "${keyring}"

	# Set priorities: Ubuntu repos highest, NVIDIA Container Toolkit next, CUDA repo blocked for driver packages
	cat <<-CHROOT_EOF > /etc/apt/preferences.d/nvidia-priority
		Package: *
		Pin: $(dirname "${mirror}")
		Pin-Priority: 400

		Package: nvidia-* libnvidia-*
		Pin: $(dirname "${mirror}")
		Pin-Priority: -1

		Package: *
		Pin: origin developer.download.nvidia.com
		Pin-Priority: 800
	CHROOT_EOF

	apt update
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

cleanup_rootfs() {
	echo "chroot: Cleanup NVIDIA GPU rootfs"

	apt-mark hold libstdc++6 libzstd1 libgnutls30t64 pciutils linuxptp libnftnl11
	apt autoremove -yqq

	apt clean
	apt autoclean

	rm -rf /var/lib/apt/lists/* /var/cache/apt/* /var/log/apt /var/cache/debconf
	rm -f /etc/apt/sources.list
	rm -f /usr/bin/nvidia-ngx-updater /usr/bin/nvidia-container-runtime
	rm -f /var/log/{nvidia-installer.log,dpkg.log,alternatives.log}

	# Clear and regenerate the ld cache
	rm -f /etc/ld.so.cache
	ldconfig
}

# Start of script
echo "chroot: Setup NVIDIA GPU rootfs stage one"

set_driver_version
setup_apt_repositories
install_userspace_components
install_nvidia_fabricmanager
install_nvidia_ctk
install_nvidia_dcgm
cleanup_rootfs
