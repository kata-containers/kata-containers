#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source "/etc/os-release" || source "/usr/lib/os-release"
source "${cidir}/lib.sh"

echo "Update apt repositories"
sudo -E apt update

echo "Install chronic"
sudo -E apt install -y moreutils

declare -A packages=( \
	[kata_containers_dependencies]="libtool automake autotools-dev autoconf bc alien libpixman-1-dev coreutils" \
	[qemu_dependencies]="libcap-dev libattr1-dev libcap-ng-dev librbd-dev" \
	[nemu_dependencies]="libbrlapi0.6" \
	[kernel_dependencies]="libelf-dev flex" \
	[crio_dependencies]="libglib2.0-dev libseccomp-dev libapparmor-dev libgpgme11-dev thin-provisioning-tools" \
	[bison_binary]="bison" \
	[libudev-dev]="libudev-dev" \
	[build_tools]="build-essential python pkg-config zlib1g-dev" \
	[crio_dependencies_for_ubuntu]="libdevmapper-dev btrfs-tools util-linux" \
	[yaml_validator]="yamllint" \
	[metrics_dependencies]="smem jq" \
	[cri-containerd_dependencies]="libseccomp-dev libapparmor-dev btrfs-tools  make gcc pkg-config" \
	[crudini]="crudini" \
	[procenv]="procenv" \
	[haveged]="haveged" \
	[gnu_parallel]="parallel" \
	[libsystemd]="libsystemd-dev" \
	[redis]="redis-server" \
)

pkgs_to_install=${packages[@]}

for j in ${packages[@]}; do
	pkgs=$(echo "$j")
	info "The following package will be installed: $pkgs"
	pkgs_to_install+=" $pkgs"
done
chronic sudo -E apt -y install $pkgs_to_install

if [ "$VERSION_ID" == "16.04" ] && [ "$(arch)" != "ppc64le" ]; then
	chronic sudo -E add-apt-repository ppa:alexlarsson/flatpak -y
	chronic sudo -E apt update
fi

echo "Install os-tree"
chronic sudo -E apt install -y libostree-dev

if [ "$(arch)" == "x86_64" ]; then
	echo "Install Kata Containers OBS repository"
	obs_url="$KATA_OBS_REPO_BASE/xUbuntu_$(lsb_release -rs)/"
	sudo sh -c "echo 'deb $obs_url /' > /etc/apt/sources.list.d/kata-containers.list"
	curl -sL  "${obs_url}/Release.key" | sudo apt-key add -
	chronic sudo -E apt-get update
fi

if [ "$KATA_KSM_THROTTLER" == "yes" ]; then
	echo "Install ${KATA_KSM_THROTTLER_JOB}"
	chronic sudo -E apt install -y ${KATA_KSM_THROTTLER_JOB}
fi
