#!/usr/bin/env bash
#
# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "${script_dir}/../../scripts/lib.sh"

readonly kernel_builder="${repo_root_dir}/tools/packaging/kernel/build-kernel.sh"

BUILDX=
PLATFORM=

DESTDIR=${DESTDIR:-${PWD}}
PREFIX=${PREFIX:-/opt/kata}
container_image="${KERNEL_CONTAINER_BUILDER:-$(get_kernel_image_name)}"

if [ "${CROSS_BUILD}" == "true" ]; then
       container_image="${container_image}-${ARCH}-cross-build"
       # Need to build a s390x image due to an issue at
       # https://github.com/kata-containers/kata-containers/pull/6586#issuecomment-1603189242
       if [ ${ARCH} == "s390x" ]; then
               BUILDX="buildx"
               PLATFORM="--platform=linux/s390x"
       fi
fi

sudo docker pull ${container_image} || \
	(sudo docker ${BUILDX} build ${PLATFORM} \
	--build-arg ARCH=${ARCH} -t "${container_image}" "${script_dir}" && \
	 # No-op unless PUSH_TO_REGISTRY is exported as "yes"
	 push_to_registry "${container_image}")

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	"${container_image}" \
	bash -c "${kernel_builder} -a ${ARCH} $* setup"

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	"${container_image}" \
	bash -c "${kernel_builder} -a ${ARCH} $* build"

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--env DESTDIR="${DESTDIR}" --env PREFIX="${PREFIX}" \
	"${container_image}" \
	bash -c "${kernel_builder} -a ${ARCH} $* install"

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--env DESTDIR="${DESTDIR}" --env PREFIX="${PREFIX}" \
	"${container_image}" \
	bash -c "${kernel_builder} -a ${ARCH} $* build-headers"
