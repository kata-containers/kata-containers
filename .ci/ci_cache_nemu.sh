#!/bin/bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

cidir=$(dirname "$0")
# Source to trap error line number
source "${cidir}/../lib/common.bash"
source "${cidir}/lib.sh"

WORKSPACE=${WORKSPACE:-$(pwd)}
nemu_version=$(get_version "assets.hypervisor.nemu.version")
nemu_tar="kata-nemu-static.tar.gz"

# This gets the current nemu version
# that we have from the runtime versions
# file this will help to compare if we have
# change the nemu version or do we have the
# same. This also creates a the
# nemu tar file with sha256sum
cache_built_nemu() {
	echo "${nemu_version}" >  "latest"
	cp -a "/tmp/${nemu_tar}" .

	sudo chown -R "${USER}:${USER}" .
	sha256sum "${nemu_tar}" >> "sha256sum-${nemu_tar}"
	cat "sha256sum-${nemu_tar}"
}

mkdir -p "${WORKSPACE}/artifacts"
pushd "${WORKSPACE}/artifacts"
rm -f "sha256sum-${nemu_tar}"
cache_built_nemu

echo "artifacts:"
ls -la "${WORKSPACE}/artifacts/"
popd
#The script is running in a VM as part of a CI Job, the artifacts will be
#collected by the CI master node, sync to make sure any data is updated.
sync
