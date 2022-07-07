#!/usr/bin/env bash
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

EXTRA_OPTS="${EXTRA_OPTS:-""}"
REMOVE_VMM_CONFIGS="${REMOVE_VMM_CONFIGS:-""}"

sudo docker build  --build-arg GO_VERSION="${GO_VERSION}" -t "${container_image}" "${script_dir}"

arch=$(uname -m)
if [ ${arch} = "ppc64le" ]; then
	arch="ppc64"
fi

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${repo_root_dir}/src/runtime" \
	"${container_image}" \
	bash -c "git config --global --add safe.directory ${repo_root_dir} && make PREFIX=${PREFIX} QEMUCMD=qemu-system-${arch} ${EXTRA_OPTS}"

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${repo_root_dir}/src/runtime" \
	"${container_image}" \
	bash -c "git config --global --add safe.directory ${repo_root_dir} && make PREFIX="${PREFIX}" DESTDIR="${DESTDIR}"  ${EXTRA_OPTS} install"

sudo sed -i -e '/^initrd =/d' "${DESTDIR}/${PREFIX}/share/defaults/kata-containers/configuration-qemu.toml"
sudo sed -i -e '/^initrd =/d' "${DESTDIR}/${PREFIX}/share/defaults/kata-containers/configuration-fc.toml"

for vmm in ${REMOVE_VMM_CONFIGS}; do
	sudo rm "${DESTDIR}/${PREFIX}/share/defaults/kata-containers/configuration-$vmm.toml"
done

pushd "${DESTDIR}/${PREFIX}/share/defaults/kata-containers"
	sudo ln -sf "configuration-qemu.toml" configuration.toml
popd
