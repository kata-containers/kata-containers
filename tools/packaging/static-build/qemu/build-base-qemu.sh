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
source "${script_dir}/../qemu.blacklist"

packaging_dir="${script_dir}/../.."
qemu_destdir="/tmp/qemu-static/"

qemu_repo="${qemu_repo:-$1}"
qemu_version="${qemu_version:-$2}"
build_suffix="${3:-}"
qemu_tar="${4:-}"

[ -n "$qemu_repo" ] || die "qemu repo not provided"
[ -n "$qemu_version" ] || die "qemu version not provided"

info "Build ${qemu_repo} version: ${qemu_version}"

http_proxy="${http_proxy:-}"
https_proxy="${https_proxy:-}"
prefix="${prefix:-"/opt/kata"}"

CACHE_TIMEOUT=$(date +"%Y-%m-%d")

sudo docker build \
	--build-arg CACHE_TIMEOUT="${CACHE_TIMEOUT}" \
	--build-arg BUILD_SUFFIX="${build_suffix}" \
	--build-arg http_proxy="${http_proxy}" \
	--build-arg https_proxy="${https_proxy}" \
	--build-arg QEMU_DESTDIR="${qemu_destdir}" \
	--build-arg QEMU_REPO="${qemu_repo}" \
	--build-arg QEMU_VERSION="${qemu_version}" \
	--build-arg QEMU_TARBALL="${qemu_tar}" \
	--build-arg PREFIX="${prefix}" \
	"${packaging_dir}" \
	-f "${script_dir}/Dockerfile" \
	-t qemu-static

sudo docker run \
	--rm \
	-i \
	-v "${PWD}":/share qemu-static \
	mv "${qemu_destdir}/${qemu_tar}" /share/

sudo chown ${USER}:${USER} "${PWD}/${qemu_tar}"
