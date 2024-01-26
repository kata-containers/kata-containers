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

init_env() {
	source "$HOME/.cargo/env"

	export LIBC=gnu

	ARCH=$(uname -m)
	rust_arch=""
	case ${ARCH} in
		"aarch64")
			rust_arch=${ARCH}
			;;
		"ppc64le")
			rust_arch="powerpc64le"
			;;
		"x86_64")
			rust_arch=${ARCH}
			;;
		"s390x")
			rust_arch=${ARCH}
			;;
	esac
	rustup target add ${rust_arch}-unknown-linux-${LIBC}
}

build_coco_guest_components_from_source() {
	echo "build coco-guest-components from source"

	init_env

	git clone --depth 1 ${coco_guest_components_repo} guest-components
	pushd guest-components

	git fetch --depth=1 origin "${coco_guest_components_version}"
	git checkout FETCH_HEAD

	TEE_PLATFORM=${TEE_PLATFORM} make build
	strip target/${rust_arch}-unknown-linux-${LIBC}/release/confidential-data-hub
	strip target/${rust_arch}-unknown-linux-${LIBC}/release/attestation-agent
	strip target/${rust_arch}-unknown-linux-${LIBC}/release/api-server-rest
	TEE_PLATFORM=${TEE_PLATFORM} make install
	popd
}

build_coco_guest_components_from_source $@
