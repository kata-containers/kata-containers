#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source /etc/os-release || source /usr/lib/os-release
source "${cidir}/lib.sh"

echo "Install chronic"
sudo -E dnf -y install moreutils

declare -A packages=( \
	[general_dependencies]="dnf-plugins-core python pkgconfig util-linux libgpg-error-devel" \
	[kata_containers_dependencies]="libtool automake autoconf bc pixman numactl-libs" \
	[qemu_dependencies]="libcap-devel libattr-devel libcap-ng-devel zlib-devel pixman-devel librbd-devel" \
	[nemu_dependencies]="brlapi" \
	[kernel_dependencies]="elfutils-libelf-devel flex" \
	[crio_dependencies]="btrfs-progs-devel device-mapper-devel glib2-devel glibc-devel glibc-static gpgme-devel libassuan-devel libseccomp-devel libselinux-devel" \
	[bison_binary]="bison" \
	[os_tree]="ostree-devel" \
	[yaml_validator]="yamllint" \
	[metrics_dependencies]="smem jq" \
	[cri-containerd_dependencies]="libseccomp-devel btrfs-progs-devel libseccomp-static" \
	[crudini]="crudini" \
	[procenv]="procenv" \
	[haveged]="haveged" \
	[gnu_parallel]="parallel" \
	[libsystemd]="systemd-devel" \
	[redis]="redis" \
)

pkgs_to_install=${packages[@]}

for j in ${packages[@]}; do
	pkgs=$(echo "$j")
	info "The following package will be installed: $pkgs"
	pkgs_to_install+=" $pkgs"
done
chronic sudo -E dnf -y install $pkgs_to_install

echo "Install kata containers dependencies"
chronic sudo -E dnf -y groupinstall "Development tools"

if [ "$(arch)" == "x86_64" ]; then
	echo "Install Kata Containers OBS repository"
	obs_url="${KATA_OBS_REPO_BASE}/Fedora_$VERSION_ID/home:katacontainers:releases:$(arch):master.repo"
	sudo -E VERSION_ID=$VERSION_ID dnf config-manager --add-repo "$obs_url"
fi

if [ "$KATA_KSM_THROTTLER" == "yes" ]; then
	echo "Install ${KATA_KSM_THROTTLER_JOB}"
	chronic sudo -E dnf -y install ${KATA_KSM_THROTTLER_JOB}
fi
