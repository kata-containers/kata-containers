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

declare -A minimal_packages=( \
	[spell-check]="hunspell myspell-en_GB myspell-en_US pandoc" \
	[yaml_validator_dependencies]="python-setuptools" \
)

declare -A packages=( \
	[general_dependencies]="curl git patch"
	[kata_containers_dependencies]="libtool automake autoconf bc libpixman-1-0-devel coreutils" \
	[qemu_dependencies]="libcap-devel libattr1 libcap-ng-devel librbd-devel" \
	[kernel_dependencies]="libelf-devel flex" \
	[crio_dependencies]="libglib-2_0-0 libseccomp-devel libapparmor-devel libgpg-error-devel glibc-devel-static libgpgme-devel libassuan-devel glib2-devel glibc-devel util-linux" \
	[bison_binary]="bison" \
	[libudev-dev]="libudev-devel" \
	[build_tools]="python zlib-devel" \
	[metrics_dependencies]="jq" \
	[cri-containerd_dependencies]="libseccomp-devel libapparmor-devel make pkg-config" \
	[haveged]="haveged" \
 	[gnu_parallel]="gnu_parallel" \
	[libsystemd]="systemd-devel" \
	[redis]="redis" \
)

main() 
{
	local setup_type="$1"
	[ -z "$setup_type" ] && die "need setup type"

	local pkgs_to_install
	local pkgs

	for pkgs in "${minimal_packages[@]}"; do
		info "The following package will be installed: $pkgs"
		pkgs_to_install+=" $pkgs"
	done

	if [ "$setup_type" = "default" ]; then
		for pkgs in "${packages[@]}"; do
			info "The following package will be installed: $pkgs"
			pkgs_to_install+=" $pkgs"
		done
	fi

	chronic sudo -E zypper -n install $pkgs_to_install

	echo "Install YAML validator"
	chronic sudo -E easy_install pip
	chronic sudo -E pip install yamllint

	[ "$setup_type" = "minimal" ] && exit 0

	echo "Install Build Tools"
	chronic sudo -E zypper -n install -t pattern "Basis-Devel"

	if [ "$(arch)" == "x86_64" ]; then
		echo "Install Kata Containers OBS repository"
		obs_url="${KATA_OBS_REPO_BASE}/SLE_${VERSION//-/_}/"
		chronic sudo -E zypper addrepo --no-gpgcheck "${obs_url}/home:katacontainers:releases:$(arch):master.repo"
	fi

	echo "Add crudini repo"
	VERSIONID="12_SP1"
	crudini_repo="https://download.opensuse.org/repositories/Cloud:OpenStack:Liberty/SLE_${VERSIONID}/Cloud:OpenStack:Liberty.repo"
	chronic sudo -E zypper addrepo --no-gpgcheck ${crudini_repo}
	chronic sudo -E zypper refresh
}

main "$@"
