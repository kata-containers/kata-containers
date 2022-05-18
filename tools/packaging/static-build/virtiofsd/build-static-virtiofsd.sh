#!/usr/bin/env bash
#
# Copyright (c) 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

ARCH=$(uname -m)

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "${script_dir}/../../scripts/lib.sh"

virtiofsd_version="${virtiofsd_version:-}"

[ -n "$virtiofsd_version" ] || virtiofsd_version=$(get_from_kata_deps "externals.virtiofsd.version")
[ -n "$virtiofsd_version" ] || die "failed to get virtiofsd version"

if [ "${ARCH}" != "x86_64" ]; then
	info "Only x86_64 binaries are distributed as part of the virtiofsd releases" && exit 1
fi

pull_virtiofsd_released_binary() {
    info "Download virtiofsd version: ${virtiofsd_version}"
    virtiofsd_zip=$(get_from_kata_deps "externals.virtiofsd.meta.binary")
    [ -n "${virtiofsd_zip}" ] || die "failed to get virtiofsd binary URL"

    mkdir -p virtiofsd

    pushd virtiofsd
    curl --fail -L ${virtiofsd_zip} -o virtiofsd.zip || return 1
    unzip virtiofsd.zip
    mv -f target/x86_64-unknown-linux-musl/release/virtiofsd virtiofsd
    chmod +x virtiofsd
    rm -rf target
    rm virtiofsd.zip
    popd
}

pull_virtiofsd_released_binary
