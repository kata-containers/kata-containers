#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

cidir=$(dirname "$0")
source "${cidir}/lib.sh"
source "${cidir}/../lib/common.bash"
source /etc/os-release || source /usr/lib/os-release

KATA_DEV_MODE="${KATA_DEV_MODE:-}"

CURRENT_QEMU_TAG=$(get_version "assets.hypervisor.qemu-experimental.tag")
QEMU_REPO_URL=$(get_version "assets.hypervisor.qemu-experimental.url")
PACKAGING_REPO="github.com/kata-containers/packaging"
QEMU_TAR="kata-qemu-static.tar.gz"
arch=$("${cidir}"/kata-arch.sh -d)
INSTALL_LOCATION="/tmp/qemu-virtiofs-static/opt/kata/bin/"
QEMU_PATH="/opt/kata/bin/qemu-virtiofs-system-x86_64"
VIRTIOFS_PATH="/opt/kata/bin/virtiofsd"

uncompress_experimental_qemu() {
	local qemu_tar_location="$1"
	[ -n "$qemu_tar_location" ] || die "provide the location of the QEMU compressed file"
	sudo tar -xf "${qemu_tar_location}" -C /
}

build_and_install_static_experimental_qemu() {
	build_experimental_qemu
	uncompress_experimental_qemu "${QEMU_TAR}"
	sudo -E ln -s "${QEMU_PATH}" "/usr/bin"
	sudo -E ln -s "${VIRTIOFS_PATH}" "/usr/bin"
}

build_experimental_qemu() {
	mkdir -p "${GOPATH}/src"
	go get -d "$PACKAGING_REPO" || true
	"${GOPATH}/src/${PACKAGING_REPO}/static-build/qemu-virtiofs/build-static-qemu-virtiofs.sh"
}

main() {
	if [ "$arch" != "x86_64" ]; then
		die "Unsupported architecture: $arch"
	fi
	mkdir -p "${QEMU_LOCATION}"
	build_and_install_static_experimental_qemu
}

main
