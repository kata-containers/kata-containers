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

cloud_hypervisor_repo="${cloud_hypervisor_repo:-}"
cloud_hypervisor_version="${cloud_hypervisor_version:-}"

if [ -z "$cloud_hypervisor_repo" ]; then
       info "Get cloud_hypervisor information from runtime versions.yaml"
       cloud_hypervisor_url=$(get_from_kata_deps "assets.hypervisor.cloud_hypervisor.url" "${kata_version}")
       [ -n "$cloud_hypervisor_url" ] || die "failed to get cloud_hypervisor url"
       cloud_hypervisor_repo="${cloud_hypervisor_url}.git"
fi
[ -n "$cloud_hypervisor_repo" ] || die "failed to get cloud_hypervisor repo"

[ -n "$cloud_hypervisor_version" ] || cloud_hypervisor_version=$(get_from_kata_deps "assets.hypervisor.cloud_hypervisor.version" "${kata_version}")
[ -n "$cloud_hypervisor_version" ] || die "failed to get cloud_hypervisor version"

pull_clh_released_binary() {
    info "Download cloud-hypervisor version: ${cloud_hypervisor_version}"
    cloud_hypervisor_binary="https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/${cloud_hypervisor_version}/cloud-hypervisor-static"

    curl --fail -L ${cloud_hypervisor_binary} -o cloud-hypervisor-static || return 1
    mkdir -p cloud-hypervisor
    mv -f cloud-hypervisor-static cloud-hypervisor/cloud-hypervisor
}

build_clh_from_source() {
    info "Build ${cloud_hypervisor_repo} version: ${cloud_hypervisor_version}"
    repo_dir=$(basename "${cloud_hypervisor_repo}")
    repo_dir="${repo_dir//.git}"
    [ -d "${repo_dir}" ] || git clone "${cloud_hypervisor_repo}"
    pushd "${repo_dir}"
    git fetch || true
    git checkout "${cloud_hypervisor_version}"
    ./scripts/dev_cli.sh build --release --libc musl
    rm -f cloud-hypervisor
    cp build/cargo_target/$(uname -m)-unknown-linux-musl/release/cloud-hypervisor .
    popd
}

if ! pull_clh_released_binary; then
    info "failed to pull cloud-hypervisor released binary, trying to build from source"
    build_clh_from_source
fi
