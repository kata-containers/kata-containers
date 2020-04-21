#!/bin/bash
#
# Copyright (c) 2019 IBM
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

CURRENT_QEMU_VERSION=$(get_version "assets.hypervisor.qemu.version")
PACKAGED_QEMU="qemu"

[ "$ID" == "ubuntu" ] || die "Unsupported distro: $ID"

get_packaged_qemu_version() {
	if [ "$ID" == "ubuntu" ]; then
		sudo apt-get update > /dev/null
		qemu_version=$(apt-cache madison $PACKAGED_QEMU \
		| awk '{print $3}' | cut -d':' -f2 | cut -d'+' -f1 | head -n 1 )
	fi

	if [ -z "$qemu_version" ]; then
		die "unknown qemu version"
	else
		echo "${qemu_version}"
	fi
}

install_packaged_qemu() {
	sudo apt install -y "$PACKAGED_QEMU"
}

build_and_install_qemu() {
	QEMU_REPO=$(get_version "assets.hypervisor.qemu.url")
	# Remove 'https://' from the repo url to be able to clone the repo using 'go get'
	QEMU_REPO_PATH=${QEMU_REPO/https:\/\//}

	PACKAGING_REPO="github.com/kata-containers/packaging"
	QEMU_CONFIG_SCRIPT="${GOPATH}/src/${PACKAGING_REPO}/scripts/configure-hypervisor.sh"

	if [ ! -d "${GOPATH}/src/${QEMU_REPO_PATH}" ]; then
		mkdir -p "${GOPATH}/src/${QEMU_REPO_PATH}"
		pushd "${GOPATH}/src/${QEMU_REPO_PATH}"
		chronic git clone "${QEMU_REPO}" "."
		popd
	fi

	go get -d "$PACKAGING_REPO" || true

	pushd "${GOPATH}/src/${QEMU_REPO_PATH}"
	git fetch
	git checkout "$CURRENT_QEMU_VERSION"
	[ -d "capstone" ] || git clone https://github.com/qemu/capstone.git capstone
	[ -d "ui/keycodemapdb" ] || git clone  https://github.com/qemu/keycodemapdb.git ui/keycodemapdb

	echo "Build Qemu"
	"${QEMU_CONFIG_SCRIPT}" "qemu" | xargs ./configure
	make -j $(nproc)

	echo "Install Qemu"
	sudo -E make install
	popd
}
