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

echo "Update apt repositories"
sudo -E apt update

echo "Install chronic"
sudo -E apt install -y moreutils

echo "Install kata containers dependencies"
chronic sudo -E apt install -y libtool automake autotools-dev autoconf bc alien libpixman-1-dev coreutils

if ! command -v docker > /dev/null; then
	"${cidir}/../cmd/container-manager/manage_ctr_mgr.sh" docker install
fi

echo "Install qemu dependencies"
chronic sudo -E apt install -y libcap-dev libattr1-dev libcap-ng-dev librbd-dev

echo "Install kernel dependencies"
chronic sudo -E apt install -y libelf-dev

echo "Install CRI-O dependencies for all Ubuntu versions"
chronic sudo -E apt install -y libglib2.0-dev libseccomp-dev libapparmor-dev \
	libgpgme11-dev go-md2man thin-provisioning-tools

echo "Install bison binary"
chronic sudo -E apt install -y bison

echo "Install libudev-dev"
chronic sudo -E apt-get install -y libudev-dev

echo "Install Build Tools"
sudo -E apt install -y build-essential python pkg-config zlib1g-dev

echo -e "Install CRI-O dependencies available for Ubuntu $VERSION_ID"
sudo -E apt install -y libdevmapper-dev btrfs-tools util-linux

if [ "$VERSION_ID" == "16.04" ]; then
	echo "Install os-tree"
	sudo -E add-apt-repository ppa:alexlarsson/flatpak -y
	sudo -E apt update
fi

sudo -E apt install -y libostree-dev

echo "Install YAML validator"
sudo -E apt install -y yamllint

echo "Install tools for metrics tests"
sudo -E apt install -y smem jq

if [ "$(arch)" == "x86_64" ]; then
	echo "Install Kata Containers OBS repository"
	obs_url="http://download.opensuse.org/repositories/home:/katacontainers:/release/xUbuntu_$(lsb_release -rs)/"
	sudo sh -c "echo 'deb $obs_url /' > /etc/apt/sources.list.d/kata-containers.list"
	curl -sL  "${obs_url}/Release.key" | sudo apt-key add -
	sudo -E apt-get update
fi

echo -e "Install cri-containerd dependencies"
sudo -E apt install -y libseccomp-dev libapparmor-dev btrfs-tools  make gcc pkg-config
