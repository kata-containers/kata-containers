#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# Currently we will use this repository until this issue is solved
# See https://github.com/kata-containers/packaging/issues/1

set -e

cidir=$(dirname "$0")
source "${cidir}/lib.sh"
source "/etc/os-release"

repo_name="packaging"
repo_owner="kata-containers"
kata_kernel_dir="/usr/share/kata-containers"
kernel_arch="$(arch)"
get_kernel_url="https://cdn.kernel.org/pub/linux/kernel"
tmp_dir="$(mktemp -d)"
hypervisor="kvm"
packaged_kernel="kata-linux-container"

download_repo() {
	pushd ${tmp_dir}
	git clone --depth 1 https://github.com/${repo_owner}/${repo_name}
	popd
}

get_current_kernel_version() {
	kernel_version=$(get_version "assets.kernel.version")
	echo "${kernel_version/v/}"
}

get_kata_config_version() {
	pushd "${tmp_dir}/${repo_name}" >> /dev/null
	kata_config_version=$(cat kernel/kata_config_version)
	popd >> /dev/null
	echo "${kata_config_version}"
}

get_packaged_kernel_version() {
	if [ "$ID" == "ubuntu" ]; then
		kernel_version=$(sudo apt-cache madison $packaged_kernel | awk '{print $3}' | cut -d'-' -f1)
	elif [ "$ID" == "fedora" ]; then
		kernel_version=$(sudo dnf --showduplicate list ${packaged_kernel}.${kernel_arch} | awk '/'$packaged_kernel'/ {print $2}' | cut -d'-' -f1)
	elif [ "$ID" == "centos" ]; then
		kernel_version=$(sudo yum --showduplicate list $packaged_kernel | awk '/'$packaged_kernel'/ {print $2}' | cut -d'-' -f1)
	fi

	echo "${kernel_version}"
}

# download the linux kernel, first argument is the kernel version
download_kernel() {
	kernel_version=$1
	pushd $tmp_dir
	kernel_tar_file="linux-${kernel_version}.tar.xz"
	kernel_url="${get_kernel_url}/v$(echo $kernel_version | cut -f1 -d.).x/${kernel_tar_file}"
	curl -LOk ${kernel_url}
	tar -xf ${kernel_tar_file}
	popd
}

# build the linux kernel, first argument is the kernel version
build_and_install_kernel() {
	kernel_version=$1
	download_kernel ${kernel_version}
	pushd ${tmp_dir}
	kernel_config_file=$(realpath ${repo_name}/kernel/configs/[${kernel_arch}]*_kata_${hypervisor}_* | tail -1)
	kernel_patches=$(realpath ${repo_name}/kernel/patches/*)
	kernel_src_dir="linux-${kernel_version}"
	pushd ${kernel_src_dir}
	cp ${kernel_config_file} .config
	for p in ${kernel_patches}; do patch -p1 < $p; done
	make -s ARCH=${kernel_arch} oldconfig > /dev/null
	if [ $CI == "true" ]; then
		make ARCH=${kernel_arch} -j$(nproc)
	else
		make ARCH=${kernel_arch}
	fi
	sudo mkdir -p ${kata_kernel_dir}
	sudo cp -a "$(realpath arch/${kernel_arch}/boot/bzImage)" "${kata_kernel_dir}/vmlinuz.container"
	sudo cp -a "$(realpath vmlinux)" "${kata_kernel_dir}/vmlinux.container"
	popd
	popd

	cleanup
}

install_packaged_kernel(){
	rc=0
	if [ "$ID"  == "ubuntu" ]; then
		chronic sudo apt install -y "$packaged_kernel" || rc=1
	elif [ "$ID"  == "fedora" ]; then
		chronic sudo dnf install -y "$packaged_kernel" || rc=1
	elif [ "$ID"  == "centos" ]; then
		chronic sudo yum install -y "$packaged_kernel" || rc=1
	else
		die "Unrecognized distro"
	fi

	return "$rc"
}

cleanup() {
	rm -rf "${tmp_dir}"
}

main() {
	download_repo
	kernel_version="$(get_current_kernel_version)"
	kata_config_version="$(get_kata_config_version)"
	current_kernel_version="${kernel_version}.${kata_config_version}"
	packaged_kernel_version=$(get_packaged_kernel_version)
	if [ "$packaged_kernel_version" == "$current_kernel_version" ] && [ "$kernel_arch" == "x86_64" ]; then
		# If installing packaged kernel from OBS fails,
		# then build and install it from sources.
		install_packaged_kernel || \
			build_and_install_kernel ${kernel_version}

	else
		build_and_install_kernel ${kernel_version}
	fi
}

main
