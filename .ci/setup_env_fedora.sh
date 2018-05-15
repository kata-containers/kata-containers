#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source /etc/os-release
source "${cidir}/lib.sh"

echo "Install chronic"
sudo -E dnf -y install moreutils

if ! command -v docker > /dev/null; then
	"${cidir}/../cmd/container-manager/manage_ctr_mgr.sh" docker install
fi

chronic sudo -E dnf -y install dnf-plugins-core
chronic sudo -E dnf makecache

echo "Install test dependencies"
chronic sudo -E dnf -y install python

echo "Install kata containers dependencies"
chronic sudo -E dnf -y groupinstall "Development tools"
chronic sudo -E dnf -y install libtool automake autoconf bc pixman numactl-libs

echo "Install qemu dependencies"
chronic sudo -E dnf -y install libcap-devel libattr-devel \
	libcap-ng-devel zlib-devel pixman-devel librbd-devel

echo "Install kernel dependencies"
chronic sudo -E dnf -y install elfutils-libelf-devel

echo "Install kata containers image"
"${cidir}/install_kata_image.sh"

echo "Install CRI-O dependencies"
chronic sudo -E dnf -y install btrfs-progs-devel device-mapper-devel      \
	glib2-devel glibc-devel glibc-static gpgme-devel libassuan-devel  \
	libgpg-error-devel libseccomp-devel libselinux-devel ostree-devel \
	pkgconfig go-md2man util-linux

echo "Install bison binary"
chronic sudo -E dnf -y install bison

echo "Install YAML validator"
sudo -E dnf -y install yamllint

echo "Install tools for metrics tests"
sudo -E dnf -y install smem jq
