#!/bin/bash
#
# Copyright (c) 2018-2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source "/etc/os-release" || "source /usr/lib/os-release"
source "${cidir}/lib.sh"

# Obtain CentOS version
if [ -f /etc/os-release ]; then
  centos_version=$(grep VERSION_ID /etc/os-release | cut -d '"' -f2)
else
  centos_version=$(grep VERSION_ID /usr/lib/os-release | cut -d '"' -f2)
fi

# Send error when a package is not available in the repositories
echo "skip_missing_names_on_install=0" | sudo tee -a /etc/yum.conf

# Check EPEL repository is enabled on CentOS
if [ -z $(yum repolist | grep "Extra Packages") ]; then
	echo >&2 "ERROR: EPEL repository is not enabled on CentOS."
	# Enable EPEL repository on CentOS
	sudo -E yum install -y wget rpm
	wget https://dl.fedoraproject.org/pub/epel/epel-release-latest-${centos_version}.noarch.rpm
	sudo -E rpm -ivh epel-release-latest-${centos_version}.noarch.rpm
fi

# Enable priority to CentOS Base repo in order to
# avoid perl updating issues
if [ "$centos_version" == "8" ]; then
	sudo echo "priority=1" | sudo tee -a /etc/yum.repos.d/CentOS-Base.repo
	sudo -E yum -y clean all
fi

echo "Update repositories"
sudo -E yum -y update

if [ "$centos_version" == "8" ]; then
	echo "Enable PowerTools repository"
	sudo yum-config-manager --enable PowerTools
fi

echo "Install chronic"
sudo -E yum -y install moreutils

if [ "$centos_version" == "8" ]; then
	chronic sudo -E yum install pkgconf-pkg-config
fi 

declare -A minimal_packages=( \
	[spell-check]="hunspell hunspell-en-GB hunspell-en-US pandoc" \
	[xml_validator]="libxml2" \
	[yaml_validator]="yamllint" \
)

declare -A packages=( \
	[kata_containers_dependencies]="libtool libtool-ltdl-devel device-mapper-persistent-data lvm2 libtool-ltdl" \
	[qemu_dependencies]="libcap-devel libcap-ng-devel libattr-devel libcap-ng-devel librbd1-devel flex libfdt-devel" \
	[kernel_dependencies]="elfutils-libelf-devel flex pkgconfig patch" \
	[crio_dependencies]="libassuan-devel libgpg-error-devel device-mapper-libs util-linux libselinux-devel" \
	[bison_binary]="bison" \
	[libgudev1-dev]="libgudev1-devel" \
	[general_dependencies]="gpgme-devel glib2-devel glibc-devel bzip2 m4 gettext-devel automake autoconf pixman-devel coreutils" \
	[build_tools]="python3 pkgconfig zlib-devel" \
	[ostree]="ostree-devel" \
	[metrics_dependencies]="jq" \
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
		info "The following package will be installed: $pkgs"
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
		chronic sudo -E yum install ${KATA_KSM_THROTTLER_JOB}
	fi
}

main "$@"
