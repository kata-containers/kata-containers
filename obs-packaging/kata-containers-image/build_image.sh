#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
set -x

set -o errexit
set -o nounset
set -o pipefail

tmp_dir=$(mktemp -d -t build-image-tmp.XXXXXXXXXX)

script_dir=$(dirname "$0")
source ${script_dir}/../versions.txt


readonly OSBUILDER_URL=https://github.com/kata-containers/osbuilder.git
AGENT_SHA="$kata_agent_hash"

#Image information
IMG_DISTRO="${osbuilder_default_os:-clearlinux}"
IMG_OS_VERSION="$clearlinux_version"
CLR_BASE_URL="https://download.clearlinux.org/releases/${clearlinux_version}/clear/x86_64/os/"

#Initrd information
INITRD_DISTRO="${osbuilder_default_initrd_os:-alpine}"
INITRD_OS_VERSION="$alpine_version"

readonly IMAGE_NAME="kata-containers-image_${IMG_DISTRO}_agent_${AGENT_SHA:0:7}.img"
readonly INITRD_NAME="kata-containers-initrd_${INITRD_DISTRO}_agent_${AGENT_SHA:0:7}.initrd"

rm -f "${IMAGE_NAME}"
rm -f "${INITRD_NAME}"


pushd ${tmp_dir}
git clone $OSBUILDER_URL osbuilder
pushd osbuilder
git checkout $kata_osbuilder_version

sudo -E PATH=$PATH make initrd\
     DISTRO=$INITRD_DISTRO \
     AGENT_VERSION=$AGENT_SHA \
     OS_VERSION=$INITRD_OS_VERSION \
     DISTRO_ROOTFS="${PWD}/initrd-image" \
     USE_DOCKER=1 \
     AGENT_INIT="yes"

sudo -E PATH=$PATH make image \
     DISTRO=$IMG_DISTRO \
     AGENT_VERSION=$AGENT_SHA \
     IMG_OS_VERSION=$IMG_OS_VERSION \
     DISTRO_ROOTFS="${PWD}/rootfs-image" \
     BASE_URL=$CLR_BASE_URL

popd

popd
mv "${tmp_dir}/osbuilder/kata-containers.img" "${IMAGE_NAME}"
mv "${tmp_dir}/osbuilder/kata-containers-initrd.img" "${INITRD_NAME}"
sudo tar cfz "kata-containers.tar.gz" "${INITRD_NAME}" "${IMAGE_NAME}"

