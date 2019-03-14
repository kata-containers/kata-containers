#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

cidir=$(dirname "$0")
source "${cidir}/lib.sh"
source /etc/os-release || source /usr/lib/os-release

versions_file="${cidir}/../versions.yaml"
arch=$("${cidir}"/kata-arch.sh -d)

install_nemu() {
	local nemu_repo=$(get_version "assets.hypervisor.nemu.url")
	local nemu_version=$(get_version "assets.hypervisor.nemu.version")
	case "$arch" in
	x86_64)
		local nemu_bin="qemu-system-${arch}_virt"
		;;
	aarch64)
		local nemu_bin="qemu-system-${arch}"
		;;
	*)
		die "Unsupported architecture: $arch"
		;;
	esac

	curl -LO "${nemu_repo}/releases/download/${nemu_version}/${nemu_bin}"
	sudo install -o root -g root -m 0755 "${nemu_bin}" "/usr/local/bin"
	rm -rf "${nemu_bin}"
}

install_firmware() {
	local firmware="OVMF.fd"
	local firmware_repo=$(get_version "assets.hypervisor.nemu-ovmf.url")
	local firmware_version=$(get_version "assets.hypervisor.nemu-ovmf.version")
	local firmware_dir="/usr/share/nemu/firmware"

	sudo mkdir -p "${firmware_dir}"
	curl -LO "${firmware_repo}/releases/download/${firmware_version}/${firmware}"
	sudo install -o root -g root -m 0644 "${firmware}" "${firmware_dir}"
	rm -f "${firmware}"
}

install_nemu
install_firmware
