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
source "${cidir}/../lib/common.bash"
source "${cidir}/lib.sh"

WORKSPACE=${WORKSPACE:-$(pwd)}
CURRENT_QEMU_VERSION=$(get_version "assets.hypervisor.qemu.version")
QEMU_TAR="kata-qemu-static.tar.gz"
QEMU_SHA_FILE="sha256sum-${QEMU_TAR}"

cache_qemu_artifacts() {
	local qemu_tar_location="$1"
	[ -n "$qemu_tar_location" ] || die "couldn't retrieve QEMU location"
	echo "${CURRENT_QEMU_VERSION}" >  "latest"

	mkdir -p "${WORKSPACE}/artifacts"
	sudo chown -R "${USER}:${USER}" .
	sha256sum "${QEMU_TAR}" > "${QEMU_SHA_FILE}"
	cat "${QEMU_SHA_FILE}"
	mv "$QEMU_TAR" "$WORKSPACE/artifacts"
	mv "${QEMU_SHA_FILE}" "$WORKSPACE/artifacts"
	mv "latest" "$WORKSPACE/artifacts"
}

cache_qemu_artifacts "$QEMU_TAR"

echo "artifacts:"
ls -la "${WORKSPACE}/artifacts/"
#The script is running in a VM as part of a CI Job, the artifacts will be
#collected by the CI master node, sync to make sure any data is updated.
sync
