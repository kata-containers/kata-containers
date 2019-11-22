#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

cidir=$(dirname "$0")
tag="${1:-""}"
source /etc/os-release || source /usr/lib/os-release
source "${cidir}/lib.sh"
KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"
experimental_qemu="${experimental_qemu:-false}"

echo "Install kata-containers image"
"${cidir}/install_kata_image.sh" "${tag}"

echo "Install Kata Containers Kernel"
"${cidir}/install_kata_kernel.sh" "${tag}"

install_qemu(){
	echo "Installing qemu"
	if [ "$experimental_qemu" == "true" ]; then
		echo "Install experimental Qemu"
		"${cidir}/install_qemu_experimental.sh"
	else
		"${cidir}/install_qemu.sh"
	fi
}

case "${KATA_HYPERVISOR}" in
	"cloud-hypervisor")
		"${cidir}/install_cloud_hypervisor.sh"
		echo "Installing experimental_qemu to install virtiofsd"
		export experimental_qemu=true
		install_qemu
		;;
	"firecracker")
		"${cidir}/install_firecracker.sh"
		;;
	"qemu")
		install_qemu
		;;
	*)
		die "${KATA_HYPERVISOR} not supported for CI install"
		;;
esac

echo "Install shim"
"${cidir}/install_shim.sh" "${tag}"

echo "Install proxy"
"${cidir}/install_proxy.sh" "${tag}"

echo "Install runtime"
"${cidir}/install_runtime.sh" "${tag}"
