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
source "${script_dir}/../qemu.blacklist"

packaging_dir="${script_dir}/../.."
qemu_virtiofs_repo=$(get_from_kata_deps "assets.hypervisor.qemu-experimental.url")
# This tag will be supported on the runtime versions.yaml
qemu_virtiofs_tag=$(get_from_kata_deps "assets.hypervisor.qemu-experimental.tag")
qemu_tar="kata-qemu-static.tar.gz"
qemu_tmp_tar="kata-qemu-static-tmp.tar.gz"

info "Build ${qemu_virtiofs_repo} tag: ${qemu_virtiofs_tag}"

http_proxy="${http_proxy:-}"
https_proxy="${https_proxy:-}"
prefix="${prefix:-"/opt/kata"}"

sudo docker build \
	--no-cache \
	--build-arg http_proxy="${http_proxy}" \
	--build-arg https_proxy="${https_proxy}" \
	--build-arg QEMU_VIRTIOFS_REPO="${qemu_virtiofs_repo}" \
	--build-arg QEMU_VIRTIOFS_TAG="${qemu_virtiofs_tag}" \
	--build-arg PREFIX="${prefix}" \
	"${packaging_dir}" \
	-f "${script_dir}/Dockerfile" \
	-t qemu-virtiofs-static

sudo docker run \
	-i \
	-v "${PWD}":/share qemu-virtiofs-static \
	mv "/tmp/qemu-virtiofs-static/${qemu_tar}" /share/

sudo chown ${USER}:${USER} "${PWD}/${qemu_tar}"

# Remove blacklisted binaries
gzip -d < "${qemu_tar}" | tar --delete --wildcards -f - ${qemu_black_list[*]} | gzip > "${qemu_tmp_tar}"
mv -f "${qemu_tmp_tar}" "${qemu_tar}"
