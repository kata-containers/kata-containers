#!/bin/bash
#Copyright (c) 2018 Intel Corporation
#
#SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly build_kernel_sh="${script_dir}/build-kernel.sh"
readonly tmp_dir=$(mktemp -d -t build-kernel-tmp.XXXXXXXXXX)

exit_handler() {
	rm -rf "$tmp_dir"
}
trap exit_handler EXIT

OK() {
	echo "OK"
}

FAIL() {
	echo "FAIL: $*"
	exit -1
}

export GOPATH=${GOPATH:-$HOME/go}

source "${script_dir}/../scripts/lib.sh"

kata_kernel_version=$(get_from_kata_deps "assets.kernel.version")
kata_kernel_version=${kata_kernel_version/v/}
kernel_dir="kata-linux-${kata_kernel_version}-$(cat ${script_dir}/kata_config_version)"

check_help() {
	echo "Check help works"
	out=$(${build_kernel_sh} -h)
	[[ ${out} == *"Usage"* ]]
	OK
}

build_kernel() {
	echo "Setup a default kernel"
	out=$(${build_kernel_sh} setup 2>&1)
	[ -f "linux-${kata_kernel_version}.tar.xz" ] || FAIL "tarball does not exist"
	[ -d "${kernel_dir}" ] || FAIL "kernel directory does not exist"
	OK

	echo "Setup a default again wont download again the kernel"
	new_kernel_dir="${PWD}/kernel-kata2"
	out=$(${build_kernel_sh} -k "${new_kernel_dir}" setup 2>&1)
	[[ ${out} == *"kernel tarball already downloaded"* ]]
	[ -f "linux-${kata_kernel_version}.tar.xz" ] || FAIL "tarball does not exist"
	[ -d "${new_kernel_dir}" ] || FAIL "kernel directory does not exist"
	OK

	echo "Build default kernel"
	out=$(${build_kernel_sh} build 2>&1)
	[ $("${kata_arch_sh}" -d) != "ppc64le" ] && ([ -e "${kernel_dir}/arch/$(uname -m)/boot/bzImage" ] || FAIL "bzImage not found")
	[ -e "${kernel_dir}/vmlinux" ] || FAIL "vmlinux not found"
	OK

	echo "Install kernel"
	export DESTDIR="${tmp_dir}/kernel-install-path"
	out=$(${build_kernel_sh} install 2>&1)
	[ -e "${DESTDIR}/usr/share/kata-containers/vmlinux.container" ]
	[ -e "${DESTDIR}/usr/share/kata-containers/vmlinuz.container" ]
	unset DESTDIR
	OK
}

test_kata() {
	local cidir="${script_dir}/../.ci/"
	echo "test kata with new kernel config"
	[ -z "${CI:-}" ] && echo "skip: Not in CI" && return
	echo "Setup kernel source"
	${build_kernel_sh} setup
	echo "Build kernel"
	${build_kernel_sh} build
	echo "Install kernel"
	sudo -E PATH="$PATH" "${build_kernel_sh}" install

	source "${cidir}/lib.sh"
	pushd "${tests_repo_dir:-no-defined}"
	.ci/run.sh
	popd
}

pushd "${tmp_dir}"
check_help
build_kernel
test_kata
popd
