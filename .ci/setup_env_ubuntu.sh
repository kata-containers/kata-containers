#!/bin/bash
#
# Copyright (c) 2017-2019 Intel Corporation
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

declare -A minimal_packages=( \
	[spell-check]="hunspell hunspell-en-gb hunspell-en-us pandoc" \
	[xml_validator]="libxml2-utils" \
	[yaml_validator]="yamllint" \
)

declare -A packages=( \
	[kata_containers_dependencies]="libtool automake autotools-dev autoconf bc libpixman-1-dev coreutils" \
	[qemu_dependencies]="libcap-dev libattr1-dev libcap-ng-dev librbd-dev" \
	[kernel_dependencies]="libelf-dev flex" \
	[crio_dependencies]="libglib2.0-dev libseccomp-dev libapparmor-dev libgpgme11-dev thin-provisioning-tools" \
	[bison_binary]="bison" \
	[libudev-dev]="libudev-dev" \
	[build_tools]="build-essential python pkg-config zlib1g-dev" \
	[crio_dependencies_for_ubuntu]="libdevmapper-dev btrfs-tools util-linux" \
	[metrics_dependencies]="smem jq" \
	[cri-containerd_dependencies]="libseccomp-dev libapparmor-dev btrfs-tools  make gcc pkg-config" \
	[crudini]="crudini" \
	[procenv]="procenv" \
	[haveged]="haveged" \
	[gnu_parallel]="parallel" \
	[libsystemd]="libsystemd-dev" \
	[redis]="redis-server" \
)

if [ "$(uname -m)" == "x86_64" ] && [ "${NAME}" == "Ubuntu" ] && [ "$(echo "${VERSION_ID} >= 18.04" | bc -q)" == "1" ]; then
	packages[qemu_dependencies]+=" libpmem-dev"
fi

rust_agent_pkgs=()
rust_agent_pkgs+=("build-essential")
rust_agent_pkgs+=("g++")
rust_agent_pkgs+=("make")
rust_agent_pkgs+=("cmake")
rust_agent_pkgs+=("automake")
rust_agent_pkgs+=("autoconf")
rust_agent_pkgs+=("m4")
rust_agent_pkgs+=("libc6-dev")
rust_agent_pkgs+=("libstdc++-8-dev")
rust_agent_pkgs+=("coreutils")
rust_agent_pkgs+=("binutils")
rust_agent_pkgs+=("debianutils")
rust_agent_pkgs+=("gcc")
rust_agent_pkgs+=("musl")
rust_agent_pkgs+=("musl-dev")
rust_agent_pkgs+=("musl-tools")
rust_agent_pkgs+=("git")

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

	# packages for rust agent, build on 18.04 or later
	if [[ ! "${VERSION_ID}" < "18.04" ]]; then
		pkgs_to_install+=" ${rust_agent_pkgs[@]}"
	fi

	chronic sudo -E apt -y install $pkgs_to_install

	[ "$setup_type" = "minimal" ] && exit 0

	if [ "$VERSION_ID" == "16.04" ] && [ "$(arch)" != "ppc64le" ]; then
		chronic sudo -E add-apt-repository ppa:alexlarsson/flatpak -y
		chronic sudo -E apt update
	fi

	echo "Install os-tree"
	chronic sudo -E apt install -y libostree-dev

	if [ "$KATA_KSM_THROTTLER" == "yes" ]; then
		echo "Install ${KATA_KSM_THROTTLER_JOB}"
		chronic sudo -E apt install -y ${KATA_KSM_THROTTLER_JOB}
	fi
}

main "$@"
