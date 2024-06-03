#!/usr/bin/env bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "${script_dir}/../../scripts/lib.sh"

config_dir="${script_dir}/../../scripts/"

firecracker_url="${firecracker_url:-}"
firecracker_dir="firecracker"
firecracker_version="${firecracker_version:-}"

arch=$(uname -m)

[ -n "$firecracker_url" ] ||firecracker_url=$(get_from_kata_deps ".assets.hypervisor.firecracker.url")
[ -n "$firecracker_url" ] || die "failed to get firecracker url"

[ -n "$firecracker_version" ] || firecracker_version=$(get_from_kata_deps ".assets.hypervisor.firecracker.version")
[ -n "$firecracker_version" ] || die "failed to get firecracker version"

firecracker_tarball_url="${firecracker_url}/releases/download"

file_name="firecracker-${firecracker_version}-${arch}.tgz"
download_url="${firecracker_tarball_url}/${firecracker_version}/${file_name}"

info "Download firecracker version: ${firecracker_version} from ${download_url}"
curl -o ${file_name} -L $download_url

sha256sum="${file_name}.sha256.txt"
sha256sum_url="${firecracker_tarball_url}/${firecracker_version}/${sha256sum}"

info "Download firecracker ${sha256sum} from ${sha256sum_url}"
curl -o ${sha256sum} -L $sha256sum_url

sha256sum -c ${sha256sum}
tar zxvf ${file_name}
