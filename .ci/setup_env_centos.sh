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

echo "Update repositories"
sudo -E yum -y update

echo "Install chronic"
sudo -E yum -y install moreutils

declare -A packages=( \
	[kata_containers_dependencies]="libtool libtool-ltdl-devel device-mapper-persistent-data lvm2 device-mapper-devel libtool-ltdl" \
	[qemu_dependencies]="libcap-devel libcap-ng-devel libattr-devel libcap-ng-devel librbd1-devel flex libfdt-devel" \
	[nemu_dependencies]="brlapi" \
	[kernel_dependencies]="elfutils-libelf-devel flex pkgconfig" \
	[crio_dependencies]="glibc-static libseccomp-devel libassuan-devel libgpg-error-devel device-mapper-libs btrfs-progs-devel util-linux libselinux-devel" \
	[bison_binary]="bison" \
	[libgudev1-dev]="libgudev1-devel" \
	[general_dependencies]="gpgme-devel glib2-devel glibc-devel bzip2 m4 gettext-devel automake alien autoconf pixman-devel coreutils" \
	[build_tools]="python pkgconfig zlib-devel" \
	[ostree]="ostree-devel" \
	[yaml_validator]="yamllint" \
	[metrics_dependencies]="smem jq" \
	[cri-containerd_dependencies]="libseccomp-devel btrfs-progs-devel" \
	[crudini]="crudini" \
	[procenv]="procenv" \
	[haveged]="haveged" \
	[gnu_parallel_dependencies]="perl bzip2 make" \
	[libsystemd]="systemd-devel" \
	[redis]="redis" \
)

pkgs_to_install=

for pkgs in "${packages[@]}"; do
	info "The following package will be installed: $pkgs"
	pkgs_to_install+=" $pkgs"
done

chronic sudo -E yum -y install $pkgs_to_install

if [ "$(arch)" == "x86_64" ]; then
	echo "Install Kata Containers OBS repository"
	obs_url="${KATA_OBS_REPO_BASE}/CentOS_${VERSION_ID}/home:katacontainers:releases:$(arch):master.repo"
	sudo -E VERSION_ID=$VERSION_ID yum-config-manager --add-repo "$obs_url"
	repo_file="/etc/yum.repos.d/home\:katacontainers\:releases\:$(arch)\:master.repo"
	sudo bash -c "echo timeout=10 >> $repo_file"
	sudo bash -c "echo retries=2 >> $repo_file"
fi

echo "Install GNU parallel"
# GNU parallel not available in Centos repos, so build it instead.
build_install_parallel

if [ "$KATA_KSM_THROTTLER" == "yes" ]; then
	echo "Install ${KATA_KSM_THROTTLER_JOB}"
	chronic sudo -E yum install ${KATA_KSM_THROTTLER_JOB}
fi
