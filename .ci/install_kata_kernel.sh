#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# Currently we will use this repository until this issue is solved
# See https://github.com/kata-containers/packaging/issues/1

set -o errexit
set -o nounset
set -o pipefail

[ -z "${DEBUG:-}" ] || set -x

cidir=$(dirname "$0")
source "${cidir}/lib.sh"
source "/etc/os-release" || source "/usr/lib/os-release"

kernel_repo_name="packaging"
kernel_repo_owner="kata-containers"
kernel_repo="github.com/${kernel_repo_owner}/${kernel_repo_name}"
export GOPATH=${GOPATH:-${HOME}/go}
kernel_repo_dir="${GOPATH}/src/${kernel_repo}"
kernel_arch="$(arch)"
tmp_dir="$(mktemp -d -t install-kata-XXXXXXXXXXX)"
packaged_kernel="kata-linux-container"

exit_handler () {
	rm -rf "$tmp_dir"
}

trap exit_handler EXIT

download_repo() {
	pushd ${tmp_dir}
	go get -d -u "${kernel_repo}" || true
	popd
}

get_current_kernel_version() {
	kernel_version=$(get_version "assets.kernel.version")
	echo "${kernel_version/v/}"
}

get_kata_config_version() {
	kata_config_version=$(cat "${kernel_repo_dir}/kernel/kata_config_version")
	echo "${kata_config_version}"
}

get_packaged_kernel_version() {
	if [ "$ID" == "ubuntu" ]; then
		kernel_version=$(sudo apt-cache madison $packaged_kernel | awk '{print $3}' | cut -d'-' -f1)
	elif [ "$ID" == "fedora" ]; then
		kernel_version=$(sudo dnf --showduplicate list ${packaged_kernel}.${kernel_arch} |
			awk '/'$packaged_kernel'/ {print $2}' |
			tail -1 |
			cut -d'-' -f1)
	elif [ "$ID" == "centos" ]; then
		kernel_version=$(sudo yum --showduplicate list $packaged_kernel | awk '/'$packaged_kernel'/ {print $2}' | cut -d'-' -f1)
	fi

	echo "${kernel_version}"
}

build_and_install_kernel() {
	info "Install kernel from sources"
	pushd "${tmp_dir}" >> /dev/null
	"${kernel_repo_dir}/kernel/build-kernel.sh" "setup"
	"${kernel_repo_dir}/kernel/build-kernel.sh" "build"
	sudo -E PATH="$PATH" "${kernel_repo_dir}/kernel/build-kernel.sh" "install"
	popd >> /dev/null
}

install_packaged_kernel(){
	info "Install packaged kernel version"
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
	info "Current Kernel version ${current_kernel_version}"
	info "Get packaged kernel version"
	packaged_kernel_version=$(get_packaged_kernel_version)
	info "Packaged Kernel version ${packaged_kernel_version}"
	if [ "$packaged_kernel_version" == "$current_kernel_version" ] && [ "$kernel_arch" == "x86_64" ]; then
		# If installing packaged kernel from OBS fails,
		# then build and install it from sources.
		if ! install_packaged_kernel;then
			info "failed to install packaged kernel, trying to build from source"
			build_and_install_kernel
		fi

	else
		build_and_install_kernel
	fi
}

main
