#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "${script_dir}/../../scripts/lib.sh"

config_dir="${script_dir}/../../scripts/"

nemu_repo="${nemu_repo:-}"
nemu_version="${nemu_version:-}"
nemu_ovmf_repo="${nemu_ovmf_repo:-}"
nemu_ovmf_version="${nemu_ovmf_version:-}"

if [ -z "$nemu_repo" ]; then
	info "Get nemu information from runtime versions.yaml"
	nemu_repo=$(get_from_kata_deps "assets.hypervisor.nemu.url")
fi
[ -n "$nemu_repo" ] || die "failed to get nemu repo"

[ -n "$nemu_version" ] || nemu_version=$(get_from_kata_deps "assets.hypervisor.nemu.version")
[ -n "$nemu_version" ] || die "failed to get nemu version"

if [ -z "$nemu_ovmf_repo" ]; then
	info "Get nemu information from runtime versions.yaml"
	nemu_ovmf_repo=$(get_from_kata_deps "assets.hypervisor.nemu-ovmf.url")
	[ -n "$nemu_ovmf_repo" ] || die "failed to get nemu ovmf repo url"
fi

if [ -z "$nemu_ovmf_version" ]; then
	nemu_ovmf_version=$(get_from_kata_deps "assets.hypervisor.nemu-ovmf.version")
	[ -n "$nemu_ovmf_version" ] || die "failed to get nemu ovmf version"
fi

nemu_virtiofsd_binary="virtiofsd-x86_64"
nemu_virtiofsd_release="${nemu_repo}/releases/download/${nemu_version}"
nemu_ovmf_release="${nemu_ovmf_repo}/releases/download/${nemu_ovmf_version}/OVMF.fd"
info "Build ${nemu_repo} version: ${nemu_version}"

http_proxy="${http_proxy:-}"
https_proxy="${https_proxy:-}"
prefix="${prefix:-"/opt/kata"}"

docker build \
	--build-arg http_proxy="${http_proxy}" \
	--build-arg https_proxy="${https_proxy}" \
	--build-arg NEMU_REPO="${nemu_repo}" \
	--build-arg NEMU_VERSION="${nemu_version}" \
	--build-arg NEMU_OVMF="${nemu_ovmf_release}" \
	--build-arg VIRTIOFSD_RELEASE="${nemu_virtiofsd_release}" \
	--build-arg VIRTIOFSD="${nemu_virtiofsd_binary}" \
	--build-arg PREFIX="${prefix}" \
	"${config_dir}" \
	-f "${script_dir}/Dockerfile" \
	-t nemu-static

docker run \
	-i \
	-v "${PWD}":/share nemu-static \
	mv /tmp/nemu-static/kata-nemu-static.tar.gz /share/
