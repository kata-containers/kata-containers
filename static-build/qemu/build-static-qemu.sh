#!/bin/bash
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

qemu_repo="${qemu_repo:-}"
qemu_version="${qemu_version:-}"

if [ -z "$qemu_repo" ]; then
	info "Get qemu information from runtime versions.yaml"
	qemu_url=$(get_from_kata_deps "assets.hypervisor.qemu.url")
	[ -n "$qemu_url" ] || die "failed to get qemu url"
	qemu_repo="${qemu_url}.git"
fi
[ -n "$qemu_repo" ] || die "failed to get qemu repo"

[ -n "$qemu_version" ] || qemu_version=$(get_from_kata_deps "assets.hypervisor.qemu.version")
[ -n "$qemu_version" ] || die "failed to get qemu version"

info "Build ${qemu_repo} version: ${qemu_version}"

http_proxy="${http_proxy:-}"
https_proxy="${https_proxy:-}"

docker build \
	--build-arg http_proxy="${http_proxy}" \
	--build-arg https_proxy="${https_proxy}" \
	--build-arg QEMU_REPO="${qemu_repo}" \
	--build-arg QEMU_VERSION="${qemu_version}" \
	"${config_dir}" \
	-f "${script_dir}/Dockerfile" \
	-t qemu-static

docker run \
	-i \
	-v "${PWD}":/share qemu-static \
	mv /tmp/qemu-static/kata-qemu-static.tar.gz /share/
