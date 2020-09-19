#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir=$(dirname $(readlink -f "$0"))
kata_version="${kata_version:-}"

source "${script_dir}/../../scripts/lib.sh"

cloud_hypervisor_version="${cloud_hypervisor_version:-}"

[ -n "$cloud_hypervisor_version" ] || cloud_hypervisor_version=$(get_from_kata_deps "assets.hypervisor.cloud_hypervisor.version" "${kata_version}")
[ -n "$cloud_hypervisor_version" ] || die "failed to get cloud_hypervisor version"

info "Download cloud-hypervisor version: ${cloud_hypervisor_version}"
cloud_hypervisor_binary="https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/${cloud_hypervisor_version}/cloud-hypervisor-static"

curl --fail -L ${cloud_hypervisor_binary} -o cloud-hypervisor
