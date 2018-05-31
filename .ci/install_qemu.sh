#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source "${cidir}/lib.sh"
source /etc/os-release

CURRENT_QEMU_COMMIT=$(get_version "assets.hypervisor.qemu-lite.commit")
PACKAGED_QEMU="qemu-lite"
QEMU_ARCH=$(arch)

get_packaged_qemu_commit() {
	if [ "$ID" == "ubuntu" ]; then
		qemu_commit=$(sudo apt-cache madison $PACKAGED_QEMU \
			| awk '{print $3}' | cut -d'-' -f1 | cut -d'.' -f4)
	elif [ "$ID" == "fedora" ]; then
		qemu_commit=$(sudo dnf --showduplicate list ${PACKAGED_QEMU}.${QEMU_ARCH} \
			| awk '/'$PACKAGED_QEMU'/ {print $2}' | cut -d'-' -f1 | cut -d'.' -f4)
	elif [ "$ID" == "centos" ]; then
		qemu_commit=$(sudo yum --showduplicate list $PACKAGED_QEMU \
			| awk '/'$PACKAGED_QEMU'/ {print $2}' | cut -d'-' -f1 | cut -d'.' -f4)
	fi

	if [ -z "$qemu_commit" ]; then
		die "unknown qemu commit"
	else
		echo "${qemu_commit}"
	fi
}

install_packaged_qemu() {
	if [ "$ID"  == "ubuntu" ]; then
		sudo apt install -y "$PACKAGED_QEMU"
	elif [ "$ID"  == "fedora" ]; then
		sudo dnf install -y "$PACKAGED_QEMU"
	elif [ "$ID"  == "centos" ]; then
		sudo yum install -y "$PACKAGED_QEMU"
	else
		die "Unrecognized distro"
	fi
}

build_and_install_qemu() {
	QEMU_REPO=$(get_version "assets.hypervisor.qemu-lite.url")
	# Remove 'https://' from the repo url to be able to clone the repo using 'go get'
	QEMU_REPO=${QEMU_REPO/https:\/\//}
	PACKAGING_REPO="github.com/kata-containers/packaging"
	QEMU_CONFIG_SCRIPT="${GOPATH}/src/${PACKAGING_REPO}/scripts/configure-hypervisor.sh"

	go get -d "${QEMU_REPO}" || true
	go get -d "$PACKAGING_REPO" || true

	pushd "${GOPATH}/src/${QEMU_REPO}"
	git fetch
	git checkout "$CURRENT_QEMU_COMMIT"
	[ -d "capstone" ] || git clone https://github.com/qemu/capstone.git capstone
	[ -d "ui/keycodemapdb" ] || git clone  https://github.com/qemu/keycodemapdb.git ui/keycodemapdb

	echo "Build Qemu"
	"${QEMU_CONFIG_SCRIPT}" "qemu" | xargs ./configure
	make -j $(nproc)

	echo "Install Qemu"
	sudo -E make install

	# Add link from /usr/local/bin to /usr/bin
	sudo ln -sf $(command -v qemu-system-${QEMU_ARCH}) "/usr/bin/qemu-lite-system-${QEMU_ARCH}"
	popd
}

main() {
	packaged_qemu_commit=$(get_packaged_qemu_commit)
	short_current_qemu_commit=${CURRENT_QEMU_COMMIT:0:10}
	if [ "$packaged_qemu_commit" == "$short_current_qemu_commit" ] &&  [ "$QEMU_ARCH" == "x86_64" ]; then
		install_packaged_qemu
	else
		build_and_install_qemu
	fi
}

main
