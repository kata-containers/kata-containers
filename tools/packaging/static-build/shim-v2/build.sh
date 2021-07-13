#!/bin/bash
#
# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly repo_root_dir="$(cd "${script_dir}/../../../.." && pwd)"
readonly kernel_builder="${repo_root_dir}/tools/packaging/kernel/build-kernel.sh"


GO_VERSION=${GO_VERSION}

DESTDIR=${DESTDIR:-${PWD}}
PREFIX=${PREFIX:-/opt/kata}
container_image="shim-v2-builder"

sudo docker build  --build-arg GO_VERSION="${GO_VERSION}" -t "${container_image}" "${script_dir}"

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${repo_root_dir}/src/runtime" \
	"${container_image}" \
	bash -c "make PREFIX=${PREFIX} QEMUCMD=qemu-system-x86_64"

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${repo_root_dir}/src/runtime" \
	"${container_image}" \
	bash -c "make PREFIX="${PREFIX}" DESTDIR="${DESTDIR}" install"

sudo sed -i -e '/^initrd =/d' "${DESTDIR}/${PREFIX}/share/defaults/kata-containers/configuration-qemu.toml"
sudo sed -i -e '/^initrd =/d' "${DESTDIR}/${PREFIX}/share/defaults/kata-containers/configuration-fc.toml"

pushd "${DESTDIR}/${PREFIX}/share/defaults/kata-containers"
	sudo ln -sf "configuration-qemu.toml" configuration.toml
popd
