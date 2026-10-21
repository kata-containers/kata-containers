#!/usr/bin/env bash
#
# Copyright (c) 2026 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly td_shim_builder="${script_dir}/build-static-td-shim.sh"

# shellcheck source=/dev/null
source "${script_dir}/../../scripts/lib.sh"

ARCH=${ARCH:-$(uname -m)}
# The td-shim firmware is only consumed by Dragonball TDX guests on x86_64.
[[ "${ARCH}" != "x86_64" ]] && exit 0

DESTDIR=${DESTDIR:-${PWD}}
PREFIX=${PREFIX:-/opt/kata}
tdshim_repo="${tdshim_repo:-}"
tdshim_version="${tdshim_version:-}"

[[ -n "${tdshim_repo}" ]] || tdshim_repo=$(get_from_kata_deps ".externals.td-shim.url")
[[ -n "${tdshim_version}" ]] || tdshim_version=$(get_from_kata_deps ".externals.td-shim.version")

[[ -n "${tdshim_repo}" ]] || die "failed to get td-shim repo"
[[ -n "${tdshim_version}" ]] || die "failed to get td-shim version"

container_image="${TDSHIM_CONTAINER_BUILDER:-$(get_td_shim_image_name)}"

docker pull "${container_image}" || \
	(docker build \
		--build-arg RUST_TOOLCHAIN="$(get_from_kata_deps ".externals.td-shim.toolchain")" \
		-t "${container_image}" "${script_dir}" && \
	 # No-op unless PUSH_TO_REGISTRY is exported as "yes"
	 push_to_registry "${container_image}")

docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--env DESTDIR="${DESTDIR}" \
	--env PREFIX="${PREFIX}" \
	--env tdshim_repo="${tdshim_repo}" \
	--env tdshim_version="${tdshim_version}" \
	--env ARCH="${ARCH}" \
	--user "$(id -u)":"$(id -g)" \
	"${container_image}" \
	bash -c "${td_shim_builder}"
