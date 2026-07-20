#!/usr/bin/env bash
#
# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck disable=SC1091 # import based on variable
source "${script_dir}/../../scripts/lib.sh"

# repo_root_dir is defined in lib.sh make sure it is set
repo_root_dir="${repo_root_dir:-}"

[[ -n "${repo_root_dir}" ]] || die "repo_root_dir is not set"
readonly kernel_builder="${repo_root_dir}/tools/packaging/kernel/build-kernel.sh"

DESTDIR=${DESTDIR:-${PWD}}
PREFIX=${PREFIX:-/opt/kata}
container_image="${KERNEL_CONTAINER_BUILDER:-$(get_kernel_image_name)}"
MEASURED_ROOTFS=${MEASURED_ROOTFS:-no}
CONFIDENTIAL_GUEST=${CONFIDENTIAL_GUEST:-no}
KBUILD_SIGN_PIN="${KBUILD_SIGN_PIN:-}"
kernel_builder_args="-a ${ARCH:-} $*"
KERNEL_DEBUG_ENABLED=${KERNEL_DEBUG_ENABLED:-"no"}

if [[ "${MEASURED_ROOTFS}" == "yes" ]] || [[ "${CONFIDENTIAL_GUEST}" == "yes" ]]; then
	kernel_builder_args+=" -m"
fi

container_engine=${CONTAINER_ENGINE:-"docker"}
container_build="${container_engine} build"

"${container_engine}" pull "${container_image}" || \
	{
		${container_build} -t "${container_image}" "${script_dir}" && \
		# No-op unless PUSH_TO_REGISTRY is exported as "yes"
		push_to_registry "${container_image}";
	}

"${container_engine}" run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--env KERNEL_DEBUG_ENABLED="${KERNEL_DEBUG_ENABLED}" \
	--env KBUILD_SIGN_PIN="${KBUILD_SIGN_PIN}" \
	--user "$(id -u)":"$(id -g)" \
	"${container_image}" \
	bash -c "${kernel_builder} ${kernel_builder_args} setup"

"${container_engine}" run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--env KERNEL_DEBUG_ENABLED="${KERNEL_DEBUG_ENABLED}" \
	--user "$(id -u)":"$(id -g)" \
	"${container_image}" \
	bash -c "${kernel_builder} ${kernel_builder_args} build"

"${container_engine}" run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--env DESTDIR="${DESTDIR}" --env PREFIX="${PREFIX}" \
	--env KERNEL_DEBUG_ENABLED="${KERNEL_DEBUG_ENABLED}" \
	--user "$(id -u)":"$(id -g)" \
	"${container_image}" \
	bash -c "${kernel_builder} ${kernel_builder_args} install"

"${container_engine}" run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--env DESTDIR="${DESTDIR}" --env PREFIX="${PREFIX}" \
	--env USER="${USER}" \
	--env KBUILD_SIGN_PIN="${KBUILD_SIGN_PIN}" \
	--user "$(id -u)":"$(id -g)" \
	"${container_image}" \
	bash -c "${kernel_builder} ${kernel_builder_args} build-headers"
