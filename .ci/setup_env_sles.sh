#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source "/etc/os-release" || source "/usr/lib/os-release"
source "${cidir}/lib.sh"

echo "Add repo for perl-IPC-Run"
perl_repo="https://download.opensuse.org/repositories/devel:languages:perl/SLE_${VERSION//-/_}/devel:languages:perl.repo"
sudo -E zypper addrepo --no-gpgcheck ${perl_repo}
sudo -E zypper refresh

echo "Install perl-IPC-Run"
sudo -E zypper -n install perl-IPC-Run

echo "Add repo for moreutils"
moreutils_repo="https://download.opensuse.org/repositories/utilities/SLE_${VERSION//-/_}_Backports/utilities.repo"
sudo -E zypper addrepo --no-gpgcheck ${moreutils_repo}
sudo -E zypper refresh

echo "Install chronic"
sudo -E zypper -n install moreutils

echo "Install curl"
chronic sudo -E zypper -n install curl

echo "Install git"
chronic sudo -E zypper -n install git

echo "Install kata containers dependencies"
chronic sudo -E zypper -n install libtool automake autoconf bc libpixman-1-0-devel coreutils

echo "Install qemu dependencies"
chronic sudo -E zypper -n install libcap-devel libattr1 libcap-ng-devel librbd-devel

echo "Install kernel dependencies"
chronic sudo -E zypper -n install libelf-devel flex

echo "Install CRI-O dependencies"
chronic sudo -E zypper -n install libglib-2_0-0 libseccomp-devel libapparmor-devel libgpg-error-devel \
	glibc-devel-static libgpgme-devel libassuan-devel glib2-devel glibc-devel util-linux

echo "Install bison binary"
chronic sudo -E zypper -n install bison

echo "Install libudev-dev"
chronic sudo -E zypper -n install libudev-devel

echo "Install Build Tools"
chronic sudo -E zypper -n install -t pattern "Basis-Devel" && sudo -E zypper -n install python zlib-devel

echo "Install YAML validator"
chronic sudo -E zypper -n install python-setuptools
chronic sudo -E easy_install pip
chronic sudo -E pip install yamllint

echo "Install tools for metrics tests"
chronic sudo -E zypper -n install  jq

if [ "$(arch)" == "x86_64" ]; then
	echo "Install Kata Containers OBS repository"
	obs_url="${KATA_OBS_REPO_BASE}/SLE_${VERSION//-/_}/"
	chronic sudo -E zypper addrepo --no-gpgcheck "${obs_url}/home:katacontainers:releases:$(arch):master.repo"
fi

echo -e "Install cri-containerd dependencies"
chronic sudo -E zypper -n install libseccomp-devel libapparmor-devel make pkg-config

echo "Install patch"
chronic sudo -E zypper -n install patch

echo "Add crudini repo"
VERSIONID="12_SP1"
crudini_repo="https://download.opensuse.org/repositories/Cloud:OpenStack:Liberty/SLE_${VERSIONID}/Cloud:OpenStack:Liberty.repo"
chronic sudo -E zypper addrepo --no-gpgcheck ${crudini_repo}
chronic sudo -E zypper refresh

echo "Install crudini"
chronic sudo -E zypper -n install crudini

echo "Install haveged"
chronic sudo -E zypper -n install haveged

echo "Install GNU parallel"
chronic sudo -E zypper -n install gnu_parallel

echo "Install libsystemd"
chronic sudo -E zypper -n install systemd-devel
