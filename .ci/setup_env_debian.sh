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

declare -A minimal_packages=( \
	[spell-check]="hunspell hunspell-en-gb hunspell-en-us pandoc" \
	[xml_validator]="libxml2-utils" \
	[yaml_validator]="yamllint" \
)

declare -A packages=( \
	[general_dependencies]="curl git" \
	[kata_containers_dependencies]="libtool automake autotools-dev autoconf bc libpixman-1-dev coreutils parted" \
	[qemu_dependencies]="libcap-dev libattr1-dev libcap-ng-dev librbd-dev" \
	[kernel_dependencies]="libelf-dev flex" \
	[crio_dependencies]="libglib2.0-dev libseccomp-dev libapparmor-dev libgpgme11-dev go-md2man thin-provisioning-tools" \
	[bison_binary]="bison" \
	[libudev-dev]="libudev-dev" \
	[build_tools]="build-essential python pkg-config zlib1g-dev" \
	[crio_dependencies_for_debian]="libdevmapper-dev btrfs-tools util-linux" \
	[os_tree]="libostree-dev" \
	[metrics_dependencies]="jq" \
	[cri-containerd_dependencies]="libseccomp-dev libapparmor-dev btrfs-tools  make gcc pkg-config" \
	[crudini]="crudini" \
	[procenv]="procenv" \
	[haveged]="haveged" \
	[gnu_parallel]="parallel" \
	[libsystemd]="libsystemd-dev"\
	[redis]="redis-server" \
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

	chronic sudo -E apt -y install $pkgs_to_install

	[ "$setup_type" = "minimal" ] && exit 0

	if [ "$KATA_KSM_THROTTLER" == "yes" ]; then
		echo "Install ${KATA_KSM_THROTTLER_JOB}"
		chronic sudo -E apt install -y ${KATA_KSM_THROTTLER_JOB}
	fi
}

main "$@"
