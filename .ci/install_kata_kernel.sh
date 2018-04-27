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

repo_name="packaging"
repo_owner="kata-containers"
kata_kernel_dir="/usr/share/kata-containers"
kernel_arch="$(arch)"
get_kernel_url="https://cdn.kernel.org/pub/linux/kernel"
tmp_dir="$(mktemp -d)"
hypervisor="kvm"

download_repo() {
	pushd ${tmp_dir}
	git clone --depth 1 https://github.com/${repo_owner}/${repo_name}
	popd
}

get_kernel_version() {
	pushd $tmp_dir/$repo_name > /dev/null
	kernel_version=$(grep "Linux/[${kernel_arch}]*" kernel/configs/[${kernel_arch}]*_kata_${hypervisor}_* | cut -d' ' -f3 | tail -1)
	popd > /dev/null
	if [ -z "$kernel_version" ]; then
		die "unknown kernel version for architecture $kernel_arch"
	else
		echo ${kernel_version}
	fi
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
}

cleanup() {
	rm -rf "${tmp_dir}"
}

main() {
	download_repo
	kernel_version=$(get_kernel_version)
	download_kernel ${kernel_version}
	build_and_install_kernel ${kernel_version}
	cleanup
}

main
