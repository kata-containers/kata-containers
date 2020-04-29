#!/bin/bash
#
# Copyright (c) 2017,2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

SCRIPT_PATH=$(dirname $(readlink -f $0))

if [ ! $(command -v go) ]; then
	echo "This script requires go to be installed and executable"
	exit 1
fi

GOPATH=$(go env "GOPATH")

if [ ! $(command -v docker) ]; then
	echo "This script requires docker to be installed and executable"
	exit 1
fi

if [ ! $(command -v git) ]; then
	echo "This script requires git to be installed and executable"
	exit 1
fi

tmpdir=$(mktemp -d)
virtcontainers_build_dir="virtcontainers/build"
echo -e "Create temporary build directory ${tmpdir}/${virtcontainers_build_dir}"
mkdir -p ${tmpdir}/${virtcontainers_build_dir}

TMPDIR="${SCRIPT_PATH}/supportfiles"
OPTDIR="/opt"
ETCDIR="/etc"

echo -e "Create ${TMPDIR}/cni/bin (needed by testing)"
rm -rf ${TMPDIR}/cni/bin
mkdir -p ${TMPDIR}/cni/bin
echo -e "Create cni directories ${OPTDIR}/cni/bin and ${ETCDIR}/cni/net.d"
sudo mkdir -p ${OPTDIR}/cni/bin
sudo mkdir -p ${ETCDIR}/cni/net.d

bundlesdir="${TMPDIR}/bundles"
echo -e "Create bundles in ${bundlesdir}"
rm -rf ${bundlesdir}
busybox_bundle="${bundlesdir}/busybox"
mkdir -p ${busybox_bundle}
docker pull busybox
pushd ${busybox_bundle}
rootfsdir="rootfs"
mkdir ${rootfsdir}
docker export $(docker create busybox) | tar -C ${rootfsdir} -xvf -
echo -e '#!/bin/sh\ncd "\"\n"sh"' > ${rootfsdir}/.containerexec
echo -e 'HOME=/root\nPATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin\nTERM=xterm' > ${rootfsdir}/.containerenv
popd

echo -e "Move to ${tmpdir}/${virtcontainers_build_dir}"
pushd ${tmpdir}/${virtcontainers_build_dir}
echo "Clone cni"
git clone https://github.com/containernetworking/plugins.git
pushd plugins
git checkout 7f98c94613021d8b57acfa1a2f0c8d0f6fd7ae5a

echo "Copy CNI config files"
cp $GOPATH/src/github.com/kata-containers/runtime/virtcontainers/test/cni/10-mynet.conf ${ETCDIR}/cni/net.d/
cp $GOPATH/src/github.com/kata-containers/runtime/virtcontainers/test/cni/99-loopback.conf ${ETCDIR}/cni/net.d/

./build.sh
cp ./bin/bridge ${TMPDIR}/cni/bin/cni-bridge
cp ./bin/loopback ${TMPDIR}/cni/bin/loopback
cp ./bin/host-local ${TMPDIR}/cni/bin/host-local
popd
popd
sudo cp ${TMPDIR}/cni/bin/* ${OPTDIR}/cni/bin/

rm -rf ${tmpdir}
