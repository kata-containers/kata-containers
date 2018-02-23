#!/bin/bash
#
# Copyright (c) 2017,2018 Intel Corporation

# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#      http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#

set -e

SCRIPT_PATH=$(dirname $(readlink -f $0))

if [[ -z "$GOPATH" ]]; then
	echo "This script requires GOPATH to be set. You may need to invoke via 'sudo -E PATH=$PATH ./virtcontainers-setup.sh'"
	exit 1
fi

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

echo "Copy CNI config files"
sudo cp $GOPATH/src/github.com/containers/virtcontainers/test/cni/10-mynet.conf ${ETCDIR}/cni/net.d/
sudo cp $GOPATH/src/github.com/containers/virtcontainers/test/cni/99-loopback.conf ${ETCDIR}/cni/net.d/

pushd plugins
./build.sh
cp ./bin/bridge ${TMPDIR}/cni/bin/cni-bridge
cp ./bin/loopback ${TMPDIR}/cni/bin/loopback
cp ./bin/host-local ${TMPDIR}/cni/bin/host-local
popd
popd
sudo cp ${TMPDIR}/cni/bin/* ${OPTDIR}/cni/bin/

rm -rf ${tmpdir}
