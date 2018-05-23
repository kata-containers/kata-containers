#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

cidir=$(dirname "$0")

tmp_dir=$(mktemp -d -t kata-image-install.XXXXXXXXXX)
readonly ROOTFS_DIR="${tmp_dir}/rootfs"
export ROOTFS_DIR

finish() {
  [ -d "${ROOTFS_DIR}" ] && [[ "${ROOTFS_DIR}" = *"rootfs"* ]] && sudo rm -rf "${ROOTFS_DIR}"
  rm -rf "$tmp_dir"
}

trap finish EXIT

OSBUILDER_DISTRO=${OSBUILDER_DISTRO:-clearlinux}
AGENT_INIT=${AGENT_INIT:-no}
TEST_INITRD=${TEST_INITRD:-no}

# Build Kata agent
bash -f ${cidir}/install_agent.sh

osbuilder_repo="github.com/kata-containers/osbuilder"

# Clone os-builder repository
go get -d ${osbuilder_repo} || true

pushd "${GOPATH}/src/${osbuilder_repo}/rootfs-builder"
sudo -E AGENT_INIT=${AGENT_INIT} GOPATH=$GOPATH USE_DOCKER=true ./rootfs.sh ${OSBUILDER_DISTRO}
popd

# Build the image
if [ x"${TEST_INITRD}" == x"yes" ]; then
    pushd "${GOPATH}/src/${osbuilder_repo}/initrd-builder"
    sudo -E AGENT_INIT=${AGENT_INIT} USE_DOCKER=true ./initrd_builder.sh "$ROOTFS_DIR"
    image_name="kata-containers-initrd.img"
else
    pushd "${GOPATH}/src/${osbuilder_repo}/image-builder"
    sudo -E AGENT_INIT=${AGENT_INIT} USE_DOCKER=true ./image_builder.sh "$ROOTFS_DIR"
    image_name="kata-containers.img"
fi

# Install the image
agent_commit=$(git --work-tree=$GOPATH/src/github.com/kata-containers/agent/ --git-dir=$GOPATH/src/github.com/kata-containers/agent/.git log --format=%h -1 HEAD)
commit=$(git log --format=%h -1 HEAD)
date=$(date +%Y-%m-%d-%T.%N%z)
image="kata-containers-${date}-osbuilder-${commit}-agent-${agent_commit}"

sudo install -o root -g root -m 0640 -D ${image_name} "/usr/share/kata-containers/${image}"
(cd /usr/share/kata-containers && sudo ln -sf "$image" ${image_name})

popd
