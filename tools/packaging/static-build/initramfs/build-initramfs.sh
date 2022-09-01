#!/bin/bash
#
# Copyright (c) 2022 Intel
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${script_dir}/../../scripts/lib.sh"
install_dir="${1:-.}"

cryptsetup_repo="${cryptsetup_repo:-}"
cryptsetup_version="${cryptsetup_version:-}"
lvm2_repo="${lvm2_repo:-}"
lvm2_version="${lvm2_version:-}"

[ -n "${cryptsetup_repo}" ] || die "Failed to get cryptsetup repo"
[ -n "${cryptsetup_version}" ] || die "Failed to get cryptsetup version"
[ -n "${lvm2_repo}" ] || die "Failed to get lvm2 repo"
[ -n "${lvm2_version}" ] || die "Failed to get lvm2 version"

build_root=$(mktemp -d)
pushd ${build_root}

info "Build ${lvm2_repo} version: ${lvm2_version}"
git clone --depth 1 --branch "${lvm2_version}" "${lvm2_repo}" lvm2
pushd lvm2
./configure --enable-static_link --disable-selinux
make && make install
cp ./libdm/libdevmapper.pc /usr/lib/pkgconfig/devmapper.pc
popd #lvm2

info "Build ${cryptsetup_repo} version: ${cryptsetup_version}"
git clone --depth 1 --branch "${cryptsetup_version}" "${cryptsetup_repo}" cryptsetup
pushd cryptsetup
./autogen.sh
./configure  --enable-static --enable-static-cryptsetup --disable-udev --disable-external-tokens --disable-ssh-token
make && make install
strip /usr/sbin/veritysetup.static
popd #cryptsetup

info "Build gen_init_cpio tool"
git clone --depth 1 --filter=blob:none --sparse https://github.com/torvalds/linux.git
pushd linux
git sparse-checkout add usr && cd usr && make gen_init_cpio
install gen_init_cpio /usr/sbin/
popd #linux

popd #${build_root}

install "${script_dir}/init.sh" /usr/sbin/
gen_init_cpio "${script_dir}/initramfs.list" | gzip -9 -n > "${install_dir}"/initramfs.cpio.gz
