#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

QEMU_REPO="github.com/qemu/qemu"
QEMU_DIR="qemu"
KATA_QEMU_BRANCH="stable-2.11"
PACKAGING_REPO="github.com/kata-containers/packaging"
QEMU_CONFIG_SCRIPT="configure-hypervisor.sh"

git clone "https://${QEMU_REPO}"

# Get qemu configuration script and copy to
# the qemu repository
go get -d "$PACKAGING_REPO" || true
cp "${GOPATH}/src/${PACKAGING_REPO}/scripts/${QEMU_CONFIG_SCRIPT}" "${QEMU_DIR}"

pushd "$QEMU_DIR"
git checkout "$KATA_QEMU_BRANCH"

echo "Build Qemu"
eval "./${QEMU_CONFIG_SCRIPT}" "qemu" | xargs ./configure
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
