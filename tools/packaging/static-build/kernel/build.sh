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
readonly initramfs_builder="${repo_root_dir}/tools/packaging/static-build/initramfs/build.sh"

BUILDX=
PLATFORM=

DESTDIR=${DESTDIR:-${PWD}}
PREFIX=${PREFIX:-/opt/kata}
container_image="${KERNEL_CONTAINER_BUILDER:-$(get_kernel_image_name)}"
MEASURED_ROOTFS=${MEASURED_ROOTFS:-no}
kernel_builder_args="-a ${ARCH} $*"

if [ "${MEASURED_ROOTFS}" == "yes" ]; then
	info "build initramfs for cc kernel"
	"${initramfs_builder}"
	# Turn on the flag to build the kernel with support to
	# measured rootfs.
	kernel_builder_args+=" -m"
fi

if [ "${CROSS_BUILD}" == "true" ]; then
       container_image="${container_image}-${ARCH}-cross-build"
       # Need to build a s390x image due to an issue at
       # https://github.com/kata-containers/kata-containers/pull/6586#issuecomment-1603189242
       if [ ${ARCH} == "s390x" ]; then
               BUILDX="buildx"
               PLATFORM="--platform=linux/s390x"
       fi
fi

docker pull ${container_image} || \
	(docker ${BUILDX} build ${PLATFORM} \
	--build-arg ARCH=${ARCH} -t "${container_image}" "${script_dir}" && \
	 # No-op unless PUSH_TO_REGISTRY is exported as "yes"
	 push_to_registry "${container_image}")

docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--user "$(id -u)":"$(id -g)" \
	"${container_image}" \
	bash -c "${kernel_builder} ${kernel_builder_args} setup"

docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--user "$(id -u)":"$(id -g)" \
	"${container_image}" \
	bash -c "${kernel_builder} ${kernel_builder_args} build"

docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--env DESTDIR="${DESTDIR}" --env PREFIX="${PREFIX}" \
	--user "$(id -u)":"$(id -g)" \
	"${container_image}" \
	bash -c "${kernel_builder} ${kernel_builder_args} install"

docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--env DESTDIR="${DESTDIR}" --env PREFIX="${PREFIX}" \
	--env USER="${USER}" \
	--user "$(id -u)":"$(id -g)" \
	"${container_image}" \
	bash -c "${kernel_builder} ${kernel_builder_args} build-headers"
