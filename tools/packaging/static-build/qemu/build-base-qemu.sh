#!/usr/bin/env bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly qemu_builder="${script_dir}/build-qemu.sh"

source "${script_dir}/../../scripts/lib.sh"
source "${script_dir}/../qemu.blacklist"

ARCH=${ARCH:-$(uname -m)}
dpkg_arch=":${ARCH}"
[ ${dpkg_arch} == ":aarch64" ] && dpkg_arch=":arm64"
[ ${dpkg_arch} == ":x86_64" ] && dpkg_arch=""
[ "${dpkg_arch}" == ":ppc64le" ] && dpkg_arch=":ppc64el"

packaging_dir="${script_dir}/../.."
qemu_destdir="/tmp/qemu-static/"
container_engine="${USE_PODMAN:+podman}"
container_engine="${container_engine:-docker}"

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

[ -n "${build_suffix}" ] && HYPERVISOR_NAME="kata-qemu-${build_suffix}" || HYPERVISOR_NAME="kata-qemu"
[ -n "${build_suffix}" ] && PKGVERSION="kata-static-${build_suffix}" || PKGVERSION="kata-static"

container_image="${QEMU_CONTAINER_BUILDER:-$(get_qemu_image_name)}"
[ "${CROSS_BUILD}" == "true" ] && container_image="${container_image}-cross-build"

${container_engine} pull ${container_image} || ("${container_engine}" build \
	--build-arg CACHE_TIMEOUT="${CACHE_TIMEOUT}" \
	--build-arg http_proxy="${http_proxy}" \
	--build-arg https_proxy="${https_proxy}" \
	--build-arg DPKG_ARCH="${dpkg_arch}" \
	--build-arg ARCH="${ARCH}" \
	"${packaging_dir}" \
	-f "${script_dir}/Dockerfile" \
	-t "${container_image}" && \
	# No-op unless PUSH_TO_REGISTRY is exported as "yes"
	push_to_registry "${container_image}")

"${container_engine}" run --rm -i \
	--env BUILD_SUFFIX="${build_suffix}" \
	--env PKGVERSION="${PKGVERSION}" \
	--env QEMU_DESTDIR="${qemu_destdir}" \
	--env QEMU_REPO="${qemu_repo}" \
	--env QEMU_TARBALL="${qemu_tar}" \
	--env PREFIX="${prefix}" \
	--env HYPERVISOR_NAME="${HYPERVISOR_NAME}" \
	--env QEMU_VERSION_NUM="${qemu_version}" \
	--env ARCH="${ARCH}" \
	--user "$(id -u)":"$(id -g)" \
	-w "${PWD}" \
	-v "${repo_root_dir}:${repo_root_dir}" \
	-v "${PWD}":/share "${container_image}" \
	bash -c "${qemu_builder}"

