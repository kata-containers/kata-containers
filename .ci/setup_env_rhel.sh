#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source "/etc/os-release" || "source /usr/lib/os-release"
source "${cidir}/lib.sh"

echo "Add epel repository"
epel_url="https://dl.fedoraproject.org/pub/epel/epel-release-latest-7.noarch.rpm"
sudo -E yum install -y "$epel_url"

echo "Update repositories"
sudo -E yum -y update

echo "Install chronic"
sudo -E yum install -y moreutils

echo "Install kata containers dependencies"
chronic sudo -E yum install -y libtool libtool-ltdl-devel device-mapper-persistent-data lvm2 device-mapper-devel libtool-ltdl bzip2 m4 \
	patch gettext-devel automake alien autoconf bc pixman-devel coreutils

echo "Install qemu dependencies"
chronic sudo -E yum install -y libcap-devel libcap-ng-devel libattr-devel libcap-ng-devel librbd1-devel flex libfdt-devel

echo "Install nemu dependencies"
chronic sudo -E yum install -y brlapi

echo "Install kernel dependencies"
chronic sudo -E yum -y install elfutils-libelf-devel flex

echo "Install CRI-O dependencies for RHEL"
chronic sudo -E yum install -y glibc-static libseccomp-devel libassuan-devel libgpg-error-devel device-mapper-libs \
	btrfs-progs-devel util-linux gpgme-devel glib2-devel glibc-devel libselinux-devel ostree-devel \
	pkgconfig

echo "Install bison binary"
chronic sudo -E yum install -y bison

echo "Install libgudev1-devel"
chronic sudo -E yum install -y libgudev1-devel

echo "Install Build Tools"
chronic sudo -E yum install -y python pkgconfig zlib-devel ostree-devel

echo "Install YAML validator"
chronic sudo -E yum install -y yamllint

echo "Install tools for metrics tests"
chronic sudo -E yum install -y jq smem

if [ "$(arch)" == "x86_64" ]; then
	echo "Install Kata Containers OBS repository"
	obs_url="${KATA_OBS_REPO_BASE}/RHEL_7/home:katacontainers:releases:$(arch):master.repo"
	sudo -E VERSION_ID=$VERSION_ID yum-config-manager --add-repo "$obs_url"
	repo_file="/etc/yum.repos.d/home\:katacontainers\:releases\:$(arch)\:master.repo"
	sudo bash -c "echo timeout=10 >> $repo_file"
	sudo bash -c "echo retries=2 >> $repo_file"
fi

echo "Install cri-containerd dependencies"
chronic sudo -E yum install -y libseccomp-devel btrfs-progs-devel

echo "Install crudini"
chronic sudo -E yum install -y crudini

echo "Install procenv"
chronic sudo -E yum install -y procenv

echo "Install haveged"
chronic sudo -E yum install -y haveged

echo "Install GNU parallel"
# GNU parallel not available in Centos repos, so build it instead.
chronic sudo -E yum -y install perl bzip2 make
build_install_parallel

if [ "$KATA_KSM_THROTTLER" == "yes" ]; then
	echo "Install ${KATA_KSM_THROTTLER_JOB}"
	sudo -E yum install ${KATA_KSM_THROTTLER_JOB}
fi
