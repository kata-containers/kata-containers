#!/usr/bin/env bash
#
# Copyright (c) 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

kata_packaging_dir="/root/kata-containers/tools/packaging"
kata_packaging_scripts="${kata_packaging_dir}/scripts"

kata_static_build_dir="${kata_packaging_dir}/static-build"
kata_static_build_scripts="${kata_static_build_dir}/scripts"

git clone --depth=1 "${QEMU_REPO}" qemu
pushd qemu
git fetch --depth=1 origin "${QEMU_VERSION}"
git checkout FETCH_HEAD
scripts/git-submodule.sh update meson capstone
${kata_packaging_scripts}/patch_qemu.sh "${QEMU_VERSION}" "${kata_packaging_dir}/qemu/patches"
PREFIX="${PREFIX}" ${kata_packaging_scripts}/configure-hypervisor.sh -s "${HYPERVISOR_NAME}" | xargs ./configure  --with-pkgversion="${PKGVERSION}"
make -j"$(nproc +--ignore 1)"
make install DESTDIR="${QEMU_DESTDIR}"
popd
${kata_static_build_scripts}/qemu-build-post.sh
mv "${QEMU_DESTDIR}/${QEMU_TARBALL}" /share/
