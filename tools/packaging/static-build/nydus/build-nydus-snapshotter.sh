#!/bin/bash
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "${script_dir}/../../scripts/lib.sh"

ARCH=${ARCH:-$(arch_to_golang "$(uname -m)")}

if [ "$ARCH" != "x86_64" ]; then
	die "Skip build for $arch, it only works for x86_64 now."
fi

nydus_snapshotter_url="${nydus_snapshotter_url:-}"
nydus_snapshotter_version="${nydus_snapshotter_version:-}"

info "Get nydus-snapshotter information from runtime versions.yaml"
[ -n "$nydus_snapshotter_url" ] || nydus_snapshotter_url=$(get_from_kata_deps "externals.nydus-snapshotter.url")
[ -n "$nydus_snapshotter_url" ] || die "failed to get nydus-snapshotter url"
[ -n "$nydus_snapshotter_version" ] || nydus_snapshotter_version=$(get_from_kata_deps "externals.nydus-snapshotter.version")
[ -n "$nydus_snapshotter_version" ] || die "failed to get nydus-snapshotter version"

nydus_snapshotter_tarball_url="${nydus_snapshotter_url}/releases/download"

file_name="nydus-snapshotter-${nydus_snapshotter_version}-${ARCH}.tgz"
download_url="${nydus_snapshotter_tarball_url}/${nydus_snapshotter_version}/${file_name}"

info "Download nydus version: ${nydus_snapshotter_version} from ${download_url}"
curl -o ${file_name} -L $download_url

sha256sum="${file_name}.sha256sum"
sha256sum_url="${nydus_snapshotter_tarball_url}/${nydus_snapshotter_version}/${sha256sum}"

info "Download nydus snapshotter ${sha256sum} from ${sha256sum_url}"
curl -o ${sha256sum} -L $sha256sum_url

sha256sum -c ${sha256sum}
tar zxvf ${file_name}
