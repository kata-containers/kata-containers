#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source "/etc/os-release" || "source /usr/lib/os-release"
source "${cidir}/lib.sh"

# Obtain CentOS version
if [ -f /etc/os-release ]; then
  centos_version=$(grep VERSION_ID /etc/os-release | cut -d '"' -f2)
else
  centos_version=$(grep VERSION_ID /usr/lib/os-release | cut -d '"' -f2)
fi

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

echo "Install qemu dependencies"
chronic sudo -E yum install -y libcap-devel libcap-ng-devel libattr-devel libcap-ng-devel librbd1-devel flex libfdt-devel

echo "Install kernel dependencies"
chronic sudo -E yum -y install elfutils-libelf-devel

echo "Install CRI-O dependencies for CentOS"
chronic sudo -E yum install -y glibc-static libglib2.0-devel libseccomp-devel libassuan-devel libgpg-error-devel go-md2man device-mapper-libs \
	 btrfs-progs-devel util-linux gpgme-devel

echo "Install bison binary"
chronic sudo -E yum install -y bison

echo "Install libgudev1-devel"
chronic sudo -E yum install -y libgudev1-devel

echo "Install Build Tools"
chronic sudo -E yum install -y python pkgconfig zlib-devel

chronic sudo -E yum install -y ostree-devel

echo "Install YAML validator"
chronic sudo -E yum install -y yamllint

echo "Install tools for metrics tests"
chronic sudo -E yum install -y smem jq

if [ "$(arch)" == "x86_64" ]; then
	echo "Install Kata Containers OBS repository"
	obs_url="http://download.opensuse.org/repositories/home:/katacontainers:/release/CentOS_${VERSION_ID}/home:katacontainers:release.repo"
	sudo -E VERSION_ID=$VERSION_ID yum-config-manager --add-repo "$obs_url"
	repo_file="/etc/yum.repos.d/home\:katacontainers\:release.repo"
	sudo bash -c "echo timeout=10 >> $repo_file"
	sudo bash -c "echo retries=2 >> $repo_file"
fi

echo "Install cri-containerd dependencies"
chronic sudo -E yum install -y libseccomp-devel btrfs-progs-devel libseccomp-static

echo "Install crudini"
chronic sudo -E yum install -y crudini

echo "Install procenv"
chronic sudo -E yum install -y procenv

echo "Install haveged"
chronic sudo -E yum install -y haveged
