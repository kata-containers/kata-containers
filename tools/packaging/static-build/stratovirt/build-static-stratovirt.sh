#!/usr/bin/env bash
#
# Copyright (c) 2023 Huawei Technologies Co.,Ltd.
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

ARCH=$(uname -m)

# Currently, StratoVirt only support x86_64 and aarch64.
[ "${ARCH}" != "x86_64" ] && [ "${ARCH}" != "aarch64" ] && exit

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${script_dir}/../../scripts/lib.sh"

info "Get stratovirt information from runtime versions.yaml"
stratovirt_url="${stratovirt_url:-}"
[ -n "$stratovirt_url" ] || stratovirt_url=$(get_from_kata_deps ".assets.hypervisor.stratovirt.url")
[ -n "$stratovirt_url" ] || die "failed to get stratovirt url"

stratovirt_version="${stratovirt_version:-}"
[ -n "$stratovirt_version" ] || stratovirt_version=$(get_from_kata_deps ".assets.hypervisor.stratovirt.version")
[ -n "$stratovirt_version" ] || die "failed to get stratovirt version"

pull_stratovirt_released_binary() {
	file_name="stratovirt-static-${stratovirt_version##*v}-${ARCH}"
	download_url="${stratovirt_url}/releases/download/${stratovirt_version}/${file_name}.tar.gz"

	curl -L ${download_url} -o ${file_name}.tar.gz
	mkdir -p static-stratovirt
	tar zxvf ${file_name}.tar.gz -C static-stratovirt
}

pull_stratovirt_released_binary

