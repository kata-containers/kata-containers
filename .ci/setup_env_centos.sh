#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source "/etc/os-release"
source "${cidir}/lib.sh"

# Obtain CentOS version
centos_version=$(grep VERSION_ID /etc/os-release | cut -d '"' -f2)

# Check EPEL repository is enabled on CentOS
if [ -z $(yum repolist | grep "Extra Packages") ]; then
	echo >&2 "ERROR: EPEL repository is not enabled on CentOS."
	# Enable EPEL repository on CentOS
	sudo -E yum install -y wget rpm
	wget https://dl.fedoraproject.org/pub/epel/epel-release-latest-${centos_version}.noarch.rpm
	sudo -E rpm -ivh epel-release-latest-${centos_version}.noarch.rpm
fi

echo "Update repositories"
sudo -E yum -y update

echo "Install chronic"
sudo -E yum install -y moreutils

echo "Install kata containers dependencies"
chronic sudo -E yum install -y libtool libtool-ltdl-devel device-mapper-persistent-data lvm2 device-mapper-devel libtool-ltdl bzip2 m4 \
	 gettext-devel automake alien autoconf bc pixman-devel coreutils

if ! command -v docker > /dev/null; then
        "${cidir}/../cmd/container-manager/manage_ctr_mgr.sh" docker install
fi

echo "Install qemu dependencies"
chronic sudo -E yum install -y libcap-devel libcap-ng-devel libattr-devel libcap-ng-devel librbd1-devel flex

echo "Install kernel dependencies"
chronic sudo -E yum -y install elfutils-libelf-devel

echo "Install kata-containers image"
"${cidir}/install_kata_image.sh"

echo "Install CRI-O dependencies for CentOS"
chronic sudo -E yum install -y glibc-static libglib2.0-devel libseccomp-devel libassuan-devel libgpg-error-devel go-md2man device-mapper-libs \
	 btrfs-progs-devel util-linux gpgme-devel

echo "Install bison binary"
chronic sudo -E yum install -y bison

echo "Install libgudev1-devel"
chronic sudo -E yum install -y libgudev1-devel

echo "Install Build Tools"
sudo -E yum install -y python pkgconfig zlib-devel

sudo -E yum install -y ostree-devel

echo "Install YAML validator"
sudo -E yum install -y yamllint

echo "Install tools for metrics tests"
sudo -E yum install -y smem jq
