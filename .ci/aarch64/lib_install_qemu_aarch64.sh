#!/bin/bash
#
# Copyright (c) 2018 ARM Limited
#
# SPDX-License-Identifier: Apache-2.0

set -e

CURRENT_QEMU_VERSION=$(get_version "assets.hypervisor.qemu.version")
PACKAGED_QEMU="qemu"

get_packaged_qemu_version() {
        if [ "$ID" == "ubuntu" ]; then
		#output redirected to /dev/null
		sudo apt-get update > /dev/null
                qemu_version=$(apt-cache madison $PACKAGED_QEMU \
                        | awk '{print $3}' | cut -d':' -f2 | cut -d'+' -f1 | head -n 1 )
        elif [ "$ID" == "fedora" ]; then
                qemu_version=$(sudo dnf --showduplicate list ${PACKAGED_QEMU}.${QEMU_ARCH} \
                        | awk '/'$PACKAGED_QEMU'/ {print $2}' | cut -d':' -f2 | cut -d'-' -f1 | head -n 1)
		qemu_version=${qemu_version%.*}
	elif [ "$ID" == "centos" ]; then
		qemu_version=""
        fi

        if [ -z "$qemu_version" ]; then
                die "unknown qemu version"
        else
                echo "${qemu_version}"
        fi
}

install_packaged_qemu() {
        if [ "$ID"  == "ubuntu" ]; then
                sudo apt install -y "$PACKAGED_QEMU"
        elif [ "$ID"  == "fedora" ]; then
                sudo dnf install -y "$PACKAGED_QEMU"
	elif [ "$ID" == "centos" ]; then
		info "packaged qemu unsupported on centos"
        else
                die "Unrecognized distro"
        fi
}

build_and_install_qemu() {
        QEMU_REPO=$(get_version "assets.hypervisor.qemu.url")
        # Remove 'https://' from the repo url to be able to clone the repo using 'go get'
        QEMU_REPO=${QEMU_REPO/https:\/\//}
        PACKAGING_REPO="github.com/kata-containers/packaging"
        QEMU_CONFIG_SCRIPT="${GOPATH}/src/${PACKAGING_REPO}/scripts/configure-hypervisor.sh"

        go get -d "${QEMU_REPO}" || true
        go get -d "$PACKAGING_REPO" || true

        pushd "${GOPATH}/src/${QEMU_REPO}"
        git fetch
        git checkout "$CURRENT_QEMU_VERSION"
        [ -d "capstone" ] || git clone https://github.com/qemu/capstone.git capstone
        [ -d "ui/keycodemapdb" ] || git clone  https://github.com/qemu/keycodemapdb.git ui/keycodemapdb

        echo "Build Qemu"
        "${QEMU_CONFIG_SCRIPT}" "qemu" | xargs ./configure
        make -j $(nproc)

        echo "Install Qemu"
        sudo -E make install

        # Add link from /usr/local/bin to /usr/bin
        sudo ln -sf $(command -v qemu-system-${QEMU_ARCH}) "/usr/bin/qemu-system-${QEMU_ARCH}"
        popd
}

