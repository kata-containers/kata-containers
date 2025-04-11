#!/usr/bin/env bash
#
# Copyright (c) 2022 Intel
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly virtiofsd_builder="${script_dir}/build-static-virtiofsd.sh"

source "${script_dir}/../../scripts/lib.sh"

ARCH=${ARCH:-$(uname -m)}
DESTDIR=${DESTDIR:-${PWD}}
PREFIX=${PREFIX:-/opt/kata}
kata_version="${kata_version:-}"
virtiofsd_repo="${virtiofsd_repo:-}"
virtiofsd_version="${virtiofsd_version:-}"
virtiofsd_toolchain="${virtiofsd_toolchain:-}"
virtiofsd_zip="${virtiofsd_zip:-}"
package_output_dir="${package_output_dir:-}"

[ -n "${virtiofsd_repo}" ] || virtiofsd_repo=$(get_from_kata_deps ".externals.virtiofsd.url")
[ -n "${virtiofsd_version}" ] || virtiofsd_version=$(get_from_kata_deps ".externals.virtiofsd.version")
[ -n "${virtiofsd_toolchain}" ] || virtiofsd_toolchain=$(get_from_kata_deps ".externals.virtiofsd.toolchain")
[ -n "${virtiofsd_zip}" ] || virtiofsd_zip=$(get_from_kata_deps ".externals.virtiofsd.meta.binary")

[ -n "${virtiofsd_repo}" ] || die "Failed to get virtiofsd repo"
[ -n "${virtiofsd_version}" ] || die "Failed to get virtiofsd version or commit"
[ -n "${virtiofsd_toolchain}" ] || die "Failed to get the rust toolchain to build virtiofsd"

case ${ARCH} in
	"aarch64")
		libc="musl"
		;;
	"ppc64le")
		libc="gnu"
		;;
	"riscv64")
		libc="gnu"
		;;
	"s390x")
		libc="gnu"
		;;
	"x86_64")
		libc="musl"
		;;
esac

container_image="${VIRTIOFSD_CONTAINER_BUILDER:-$(get_virtiofsd_image_name)}"
[ "${CROSS_BUILD}" == "true" ] && container_image="${container_image}-cross-build"

docker pull ${container_image} || \
	(docker $BUILDX build $PLATFORM \
		--build-arg RUST_TOOLCHAIN="${virtiofsd_toolchain}" \
		-t "${container_image}" "${script_dir}/${libc}" && \
	 # No-op unless PUSH_TO_REGISTRY is exported as "yes"
	 push_to_registry "${container_image}")

docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--env DESTDIR="${DESTDIR}" \
	--env PREFIX="${PREFIX}" \
	--env virtiofsd_repo="${virtiofsd_repo}" \
	--env virtiofsd_version="${virtiofsd_version}" \
	--env virtiofsd_zip="${virtiofsd_zip}" \
	--env ARCH="${ARCH}" \
	--user "$(id -u)":"$(id -g)" \
	"${container_image}" \
	bash -c "${virtiofsd_builder}"
