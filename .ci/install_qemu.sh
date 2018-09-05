#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source "${cidir}/lib.sh"
source /etc/os-release || source /usr/lib/os-release

CURRENT_QEMU_COMMIT=$(get_version "assets.hypervisor.qemu-lite.commit")
PACKAGED_QEMU="qemu-lite"
QEMU_ARCH=$(${cidir}/kata-arch.sh -d)

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

	echo "${qemu_commit}"
}

install_packaged_qemu() {
	rc=0
	# Timeout to download packages from OBS
	limit=180
	if [ "$ID"  == "ubuntu" ]; then
		chronic sudo apt install -y "$PACKAGED_QEMU" || rc=1
	elif [ "$ID"  == "fedora" ]; then
		chronic sudo dnf install -y "$PACKAGED_QEMU" || rc=1
	elif [ "$ID"  == "centos" ]; then
		chronic sudo yum install -y "$PACKAGED_QEMU" || rc=1
	else
		die "Unrecognized distro"
	fi

	return "$rc"
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

	# Apply required patches
	QEMU_PATCHES_PATH="${GOPATH}/src/${PACKAGING_REPO}/obs-packaging/qemu-lite/patches"
	for patch in ${QEMU_PATCHES_PATH}/*.patch; do
		echo "Applying patch: $patch"
		git am -3 "$patch"
	done

	echo "Build Qemu"
	"${QEMU_CONFIG_SCRIPT}" "qemu" | xargs ./configure
	make -j $(nproc)

	echo "Install Qemu"
	sudo -E make install

	# Add link from /usr/local/bin to /usr/bin
	sudo ln -sf $(command -v qemu-system-${QEMU_ARCH}) "/usr/bin/qemu-lite-system-${QEMU_ARCH}"
	popd
}

#Load specific configure file
if [ -f "${cidir}/${QEMU_ARCH}/lib_install_qemu_${QEMU_ARCH}.sh" ]; then
	source "${cidir}/${QEMU_ARCH}/lib_install_qemu_${QEMU_ARCH}.sh"
fi

main() {
	if [ "$QEMU_ARCH" == "x86_64" ]; then
		packaged_qemu_commit=$(get_packaged_qemu_commit)
		short_current_qemu_commit=${CURRENT_QEMU_COMMIT:0:10}
		if [ "$packaged_qemu_commit" == "$short_current_qemu_commit" ]; then
			# If installing packaged qemu from OBS fails,
			# then build and install it from sources.
			install_packaged_qemu || build_and_install_qemu
		else
			build_and_install_qemu
		fi
	elif [ "$QEMU_ARCH" == "aarch64" ]; then
		packaged_qemu_version=$(get_packaged_qemu_version)
		short_current_qemu_version=${CURRENT_QEMU_VERSION#*-}
		if [ "$packaged_qemu_version" == "$short_current_qemu_version" ]; then
			install_packaged_qemu || build_and_install_qemu
		else
			build_and_install_qemu
		fi
	fi
}

main
