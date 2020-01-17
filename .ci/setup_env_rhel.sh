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

declare -A minimal_packages=( \
	[spell-check]="hunspell hunspell-en-GB hunspell-en-US pandoc" \
	[xml_validator]="libxml2" \
	[yamllint]="yamllint"
)

declare -A packages=(
	[kata_containers_dependencies]="libtool libtool-ltdl-devel device-mapper-persistent-data lvm2 device-mapper-devel libtool-ltdl bzip2 m4 patch gettext-devel automake autoconf bc pixman-devel coreutils" \
	[qemu_dependencies]="libcap-devel libcap-ng-devel libattr-devel libcap-ng-devel librbd1-devel flex libfdt-devel" \
	[kernel_dependencies]="elfutils-libelf-devel flex" \
	[crio_dependencies]="glibc-static libseccomp-devel libassuan-devel libgpg-error-devel device-mapper-libs btrfs-progs-devel util-linux gpgme-devel glib2-devel glibc-devel libselinux-devel pkgconfig" \
	[bison_binary]="bison" \
	[build_tools]="python pkgconfig zlib-devel" \
	[os_tree]="ostree-devel" \
	[libudev-dev]="libgudev1-devel" \
	[metrics_dependencies]="smem jq" \
	[cri-containerd_dependencies]="libseccomp-devel btrfs-progs-devel" \
	[crudini]="crudini" \
	[procenv]="procenv" \
	[haveged]="haveged" \
	[gnu_parallel_dependencies]="perl bzip2 make" \
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
		pkgs_to_install+=" $pkgs"
	done

	if [ "$setup_type" = "default" ]; then
		for pkgs in "${packages[@]}"; do
			info "The following package will be installed: $pkgs"
			pkgs_to_install+=" $pkgs"
		done
	fi

	chronic sudo -E yum -y install $pkgs_to_install

	[ "$setup_type" = "minimal" ] && exit 0

	echo "Install GNU parallel"
	# GNU parallel not available in Centos repos, so build it instead.
	build_install_parallel

	if [ "$KATA_KSM_THROTTLER" == "yes" ]; then
		echo "Install ${KATA_KSM_THROTTLER_JOB}"
		sudo -E yum install ${KATA_KSM_THROTTLER_JOB}
	fi
}

main "$@"
