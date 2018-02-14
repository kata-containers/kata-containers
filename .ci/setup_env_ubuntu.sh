#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source "/etc/os-release"
source "${cidir}/lib.sh"
get_cc_versions

echo "Update apt repositories"
sudo -E apt update

echo "Install chronic"
sudo -E apt install -y moreutils

echo "Install kata containers dependencies"
chronic sudo -E apt install -y libtool automake autotools-dev autoconf bc alien libpixman-1-dev coreutils

if ! command -v docker > /dev/null; then
	"${cidir}/../cmd/container-manager/manage_ctr_mgr.sh" docker install
fi

echo "Install qemu-lite binary"
"${cidir}/install_qemu_lite.sh" "${qemu_lite_clear_release}" "${qemu_lite_sha}" "$ID"

echo "Install kata-containers image"
"${cidir}/install_kata_image.sh"

echo "Install CRI-O dependencies for all Ubuntu versions"
chronic sudo -E apt install -y libglib2.0-dev libseccomp-dev libapparmor-dev libgpgme11-dev

echo "Install bison binary"
chronic sudo -E apt install -y bison

echo "Install libudev-dev"
chronic sudo -E apt-get install -y libudev-dev

echo "Install Build Tools"
sudo -E apt install -y build-essential python pkg-config zlib1g-dev

echo "Install Kata Containers Kernel"
"${cidir}/install_kata_kernel.sh" "latest"

echo -e "Install CRI-O dependencies available for Ubuntu $VERSION_ID"
sudo -E apt install -y libdevmapper-dev btrfs-tools util-linux

if [ "$VERSION_ID" == "16.04" ]; then
	echo "Install os-tree"
	sudo -E add-apt-repository ppa:alexlarsson/flatpak -y
	sudo -E apt update
fi

sudo -E apt install -y libostree-dev
