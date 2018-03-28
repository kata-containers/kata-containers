#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")

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
    sudo -E AGENT_INIT=${AGENT_INIT} USE_DOCKER=true ./initrd_builder.sh ../rootfs-builder/rootfs
    image_name="kata-containers-initrd.img"
else
    pushd "${GOPATH}/src/${osbuilder_repo}/image-builder"
    sudo -E AGENT_INIT=${AGENT_INIT} USE_DOCKER=true ./image_builder.sh ../rootfs-builder/rootfs
    image_name="kata-containers.img"
fi

# Install the image
agent_commit=$("$GOPATH/src/github.com/kata-containers/agent/kata-agent" --version | awk '{print $NF}')
commit=$(git log --format=%h -1 HEAD)
date=$(date +%Y-%m-%d-%T.%N%z)
image="kata-containers-${date}-osbuilder-${commit}-agent-${agent_commit}"

sudo install -o root -g root -m 0640 -D ${image_name} "/usr/share/kata-containers/${image}"
(cd /usr/share/kata-containers && sudo ln -sf "$image" ${image_name})

popd
