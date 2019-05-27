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

echo "Install chronic"
sudo -E zypper -n install moreutils

declare -A packages=( \
	[general_dependencies]="curl python-setuptools git libcontainers-common libdevmapper1_03 util-linux" \
	[kata_containers_dependencies]="libtool automake autoconf bc perl-Alien-SDL libpixman-1-0-devel coreutils python2-pkgconfig" \
	[qemu_dependencies]="libcap-devel libattr1 libcap-ng-devel librbd-devel" \
	[kernel_dependencies]="libelf-devel flex glibc-devel-static thin-provisioning-tools" \
	[crio_dependencies]="libglib-2_0-0 libseccomp-devel libapparmor-devel libgpg-error-devel go-md2man libgpgme-devel libassuan-devel glib2-devel glibc-devel" \
	[bison_binary]="bison" \
	[build_tools]="patterns-devel-base-devel_basis python pkg-config zlib-devel" \
	[os_tree]="libostree-devel" \
	[libudev-dev]="libudev-devel" \
	[metrics_dependencies]="smemstat jq" \
	[cri-containerd_dependencies]="libseccomp-devel libapparmor-devel make pkg-config" \
	[crudini]="crudini" \
	[haveged]="haveged" \
	[gnu_parallel]="gnu_parallel" \
	[libsystemd]="systemd-devel" \
	[redis]="redis" \
)

pkgs_to_install=${packages[@]}

for j in ${packages[@]}; do
	pkgs=$(echo "$j")
	info "The following package will be installed: $pkgs"
	pkgs_to_install+=" $pkgs"
done
chronic sudo -E zypper -n install $pkgs_to_install

echo "Install YAML validator"
chronic sudo -E easy_install pip
chronic sudo -E pip install yamllint

if [ "$(arch)" == "x86_64" ]; then
	echo "Install Kata Containers OBS repository"
	obs_url="${KATA_OBS_REPO_BASE}/openSUSE_Leap_${VERSION_ID}"
	chronic sudo -E zypper addrepo --no-gpgcheck "${obs_url}/home:katacontainers:releases:$(arch):master.repo"
fi
