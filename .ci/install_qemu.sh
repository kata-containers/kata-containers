#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

QEMU_REPO="github.com/qemu/qemu"
KATA_QEMU_BRANCH="stable-2.11"
PACKAGING_REPO="github.com/kata-containers/packaging"
QEMU_CONFIG_SCRIPT="${GOPATH}/src/${PACKAGING_REPO}/scripts/configure-hypervisor.sh"

go get -d "${QEMU_REPO}" || true

# Get qemu configuration script and copy to
# the qemu repository
go get -d "$PACKAGING_REPO" || true

pushd "${GOPATH}/src/${QEMU_REPO}"
git fetch
git checkout "$KATA_QEMU_BRANCH"
[ -d "capstone" ] || git clone https://github.com/qemu/capstone.git capstone
[ -d "ui/keycodemapdb" ] || git clone  https://github.com/qemu/keycodemapdb.git ui/keycodemapdb

echo "Build Qemu"
eval "${QEMU_CONFIG_SCRIPT}" "qemu" | xargs ./configure
make -j $(nproc)

echo "Install Qemu"
sudo -E make install

# Workaround:
# As we currently do not have a package that installs
# qemu under /usr/bin/, create a symlink.
# this should be solved when we define and have the packages
# in a repository.
sudo ln -sf $(command -v qemu-system-$(arch)) /usr/bin/

popd
