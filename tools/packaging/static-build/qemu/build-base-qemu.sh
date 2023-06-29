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

sudo docker pull ${container_image} || (sudo "${container_engine}" build \
	--build-arg CACHE_TIMEOUT="${CACHE_TIMEOUT}" \
	--build-arg http_proxy="${http_proxy}" \
	--build-arg https_proxy="${https_proxy}" \
	"${packaging_dir}" \
	-f "${script_dir}/Dockerfile" \
	-t "${container_image}" && \
	 # No-op unless PUSH_TO_REGISTRY is exported as "yes"
	 push_to_registry "${container_image}")

sudo "${container_engine}" run \
	--rm \
	-i \
	--env BUILD_SUFFIX="${build_suffix}" \
	--env HYPERVISOR_NAME="${HYPERVISOR_NAME}" \
	--env PKGVERSION="${PKGVERSION}" \
	--env QEMU_DESTDIR="${qemu_destdir}" \
	--env QEMU_REPO="${qemu_repo}" \
	--env QEMU_VERSION="${qemu_version}" \
	--env QEMU_TARBALL="${qemu_tar}" \
	--env PREFIX="${prefix}" \
	-v "${repo_root_dir}:/root/kata-containers" \
	-v "${PWD}":/share "${container_image}" \
	bash -c "/root/kata-containers/tools/packaging/static-build/qemu/build-qemu.sh"

sudo chown ${USER}:$(id -gn ${USER}) "${PWD}/${qemu_tar}"
