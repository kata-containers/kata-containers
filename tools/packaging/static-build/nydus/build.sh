#!/usr/bin/env bash
#
# Copyright (c) 2022 Ant Group
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "${script_dir}/../../scripts/lib.sh"

arch="$(uname -m)"

nydus_url="${nydus_url:-}"
nydus_version="${nydus_version:-}"

info "Get nydus information from runtime versions.yaml"
[ -n "$nydus_url" ] || nydus_url=$(get_from_kata_deps ".externals.nydus.url")
[ -n "$nydus_url" ] || die "failed to get nydus url"
[ -n "$nydus_version" ] || nydus_version=$(get_from_kata_deps ".externals.nydus.version")
[ -n "$nydus_version" ] || die "failed to get nydus version"

nydus_tarball_url="${nydus_url}/releases/download"

file_name="nydus-static-${nydus_version}-linux-$(arch_to_golang $arch).tgz"
download_url="${nydus_tarball_url}/${nydus_version}/${file_name}"

info "Download nydus version: ${nydus_version} from ${download_url}"
curl -o ${file_name} -L $download_url

sha256sum="${file_name}.sha256sum"
sha256sum_url="${nydus_tarball_url}/${nydus_version}/${sha256sum}"

info "Download nydus ${sha256sum} from ${sha256sum_url}"
curl -o ${sha256sum} -L $sha256sum_url

sha256sum -c ${sha256sum}
tar zxvf ${file_name}
