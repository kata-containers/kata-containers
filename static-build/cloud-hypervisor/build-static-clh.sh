#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir=$(dirname $(readlink -f "$0"))

source "${script_dir}/../../scripts/lib.sh"

cloud_hypervisor_repo="${cloud_hypervisor_repo:-}"
cloud_hypervisor_version="${cloud_hypervisor_version:-}"

if [ -z "$cloud_hypervisor_repo" ]; then
	info "Get cloud_hypervisor information from runtime versions.yaml"
	cloud_hypervisor_url=$(get_from_kata_deps "assets.hypervisor.cloud_hypervisor.url")
	[ -n "$cloud_hypervisor_url" ] || die "failed to get cloud_hypervisor url"
	cloud_hypervisor_repo="${cloud_hypervisor_url}.git"
fi
[ -n "$cloud_hypervisor_repo" ] || die "failed to get cloud_hypervisor repo"

[ -n "$cloud_hypervisor_version" ] || cloud_hypervisor_version=$(get_from_kata_deps "assets.hypervisor.cloud_hypervisor.version")
[ -n "$cloud_hypervisor_version" ] || die "failed to get cloud_hypervisor version"

info "Build ${cloud_hypervisor_repo} version: ${cloud_hypervisor_version}"

repo_dir=$(basename "${cloud_hypervisor_repo}")
repo_dir="${repo_dir//.git}"

[ -d "${repo_dir}" ] || git clone "${cloud_hypervisor_repo}"
cd "${repo_dir}"
git fetch || true
git checkout "${cloud_hypervisor_version}"
"${script_dir}/docker-build/build.sh"
rm -f cloud-hypervisor
cp ./target/release/cloud-hypervisor .
