#!/bin/bash
#
# Copyright (c) 2018-2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source "/etc/os-release" || source "/usr/lib/os-release"
source "${cidir}/lib.sh"
export DEBIAN_FRONTEND=noninteractive

echo "Install chronic"
sudo -E apt -y install moreutils

declare -A packages=( \
	[general_dependencies]="curl git" \
	[kata_containers_dependencies]="libtool automake autotools-dev autoconf bc alien libpixman-1-dev coreutils parted" \
	[qemu_dependencies]="libcap-dev libattr1-dev libcap-ng-dev librbd-dev" \
	[kernel_dependencies]="libelf-dev flex" \
	[crio_dependencies]="libglib2.0-dev libseccomp-dev libapparmor-dev libgpgme11-dev go-md2man thin-provisioning-tools" \
	[bison_binary]="bison" \
	[libudev-dev]="libudev-dev" \
	[build_tools]="build-essential python pkg-config zlib1g-dev" \
	[crio_dependencies_for_debian]="libdevmapper-dev btrfs-tools util-linux" \
	[os_tree]="libostree-dev" \
	[yaml_validator]="yamllint" \
	[metrics_dependencies]="smem jq" \
	[cri-containerd_dependencies]="libseccomp-dev libapparmor-dev btrfs-tools  make gcc pkg-config" \
	[crudini]="crudini" \
	[procenv]="procenv" \
	[haveged]="haveged" \
	[gnu_parallel]="parallel" \
	[libsystemd]="libsystemd-dev"\
	[redis]="redis-server" \
)

pkgs_to_install=

for pkgs in "${packages[@]}"; do
	info "The following package will be installed: $pkgs"
	pkgs_to_install+=" $pkgs"
done

chronic sudo -E apt -y install $pkgs_to_install

echo "Enable librbd1 repository"
sudo bash -c "cat <<EOF > /etc/apt/sources.list.d/unstable.list
deb http://deb.debian.org/debian unstable main contrib non-free
deb-src http://deb.debian.org/debian unstable main contrib non-free
EOF"

echo "Lower priority than stable"
sudo bash -c "cat <<EOF > /etc/apt/preferences.d/unstable
Package: *
Pin: release a=unstable
Pin-Priority: 10
EOF"

echo "Install librbd1"
chronic sudo -E apt update && sudo -E apt install -y -t unstable librbd1

if [ "$(arch)" == "x86_64" ]; then
	echo "Install Kata Containers OBS repository"
	obs_url="${KATA_OBS_REPO_BASE}/Debian_${VERSION_ID}"
	sudo sh -c "echo 'deb $obs_url /' > /etc/apt/sources.list.d/kata-containers.list"
	curl -sL  "${obs_url}/Release.key" | sudo apt-key add -
	chronic sudo -E apt-get update
fi

if [ "$KATA_KSM_THROTTLER" == "yes" ]; then
	echo "Install ${KATA_KSM_THROTTLER_JOB}"
	chronic sudo -E apt install -y ${KATA_KSM_THROTTLER_JOB}
fi
