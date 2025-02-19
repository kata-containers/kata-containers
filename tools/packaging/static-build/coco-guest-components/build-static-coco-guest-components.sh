#!/usr/bin/env bash
#
# Copyright (c) 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "${script_dir}/../../scripts/lib.sh"

[ -n "$coco_guest_components_repo" ] || die "failed to get coco-guest-components repo"
[ -n "$coco_guest_components_version" ] || die "failed to get coco-guest-components version"

[ -d "guest-components" ] && rm -rf  guest-components

build_coco_guest_components_from_source() {
	echo "build coco-guest-components from source"

	. /etc/profile.d/rust.sh

	git clone --depth 1 "${coco_guest_components_repo}" guest-components
	pushd guest-components

	git fetch --depth=1 origin "${coco_guest_components_version}"
	git checkout FETCH_HEAD

	DESTDIR="${DESTDIR}/usr/local/bin" TEE_PLATFORM=${TEE_PLATFORM} make build
	strip "target/${RUST_ARCH}-unknown-linux-${LIBC}/release/confidential-data-hub"
	strip "target/${RUST_ARCH}-unknown-linux-${LIBC}/release/attestation-agent"
	strip "target/${RUST_ARCH}-unknown-linux-${LIBC}/release/api-server-rest"
	DESTDIR="${DESTDIR}/usr/local/bin" TEE_PLATFORM=${TEE_PLATFORM} make install

	install -D -m0755 "confidential-data-hub/hub/src/storage/scripts/luks-encrypt-storage" "${DESTDIR}/usr/local/bin/luks-encrypt-storage"
	popd
}

build_coco_guest_components_from_source $@
