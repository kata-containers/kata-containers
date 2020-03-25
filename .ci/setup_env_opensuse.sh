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

# This is related with https://bugzilla.suse.com/show_bug.cgi?id=1165519
echo "Remove openSUSE cloud repo"
sudo zypper rr openSUSE-Leap-Cloud-Tools

echo "Install chronic"
sudo -E zypper -n install moreutils

declare -A minimal_packages=( \
	[spell-check]="hunspell myspell-en_GB myspell-en_US pandoc" \
	[xml_validator]="libxml2-tools" \
	[yaml_validator_dependencies]="python-setuptools" \
)

declare -A packages=( \
	[general_dependencies]="curl git libcontainers-common libdevmapper1_03 util-linux" \
	[kata_containers_dependencies]="libtool automake autoconf bc perl-Alien-SDL libpixman-1-0-devel coreutils python2-pkgconfig" \
	[qemu_dependencies]="libcap-devel libattr1 libcap-ng-devel librbd-devel libpmem-devel" \
	[kernel_dependencies]="patch libelf-devel flex glibc-devel-static thin-provisioning-tools" \
	[crio_dependencies]="libglib-2_0-0 libseccomp-devel libapparmor-devel libgpg-error-devel go-md2man libgpgme-devel libassuan-devel glib2-devel glibc-devel" \
	[bison_binary]="bison" \
	[build_tools]="gcc python pkg-config zlib-devel" \
	[os_tree]="libostree-devel" \
	[libudev-dev]="libudev-devel" \
	[metrics_dependencies]="smemstat jq" \
	[cri-containerd_dependencies]="libseccomp-devel libapparmor-devel make pkg-config libbtrfs-devel patterns-base-apparmor" \
	[crudini]="crudini" \
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
}

main "$@"
