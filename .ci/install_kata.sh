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
TEST_RUST_AGENT="${TEST_RUST_AGENT:-false}"
TEST_CGROUPSV2="${TEST_CGROUPSV2:-false}"

if [ "${TEST_RUST_AGENT}" == true ]; then
	echo "Install rust agent image"
	"${cidir}/install_kata_image_rust.sh"
else
	echo "Install kata-containers image"
	"${cidir}/install_kata_image.sh" "${tag}"
fi

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

echo "Install shim"
"${cidir}/install_shim.sh" "${tag}"

echo "Install proxy"
"${cidir}/install_proxy.sh" "${tag}"

echo "Install runtime"
"${cidir}/install_runtime.sh" "${tag}"

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

if [ "${TEST_CGROUPSV2}" == "true" ]; then
	echo "Configure podman with kata"
	"${cidir}/configure_podman_for_kata.sh"
fi

# Check system supports running Kata Containers
kata_runtime_path=$(command -v kata-runtime)
sudo -E PATH=$PATH "$kata_runtime_path" kata-check
