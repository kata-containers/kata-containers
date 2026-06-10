#!/usr/bin/env bash
#
# Copyright (c) 2024 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly nvrc_builder="${script_dir}/build-static-nvrc.sh"

# shellcheck source=/dev/null
source "${script_dir}/../../scripts/lib.sh"

DESTDIR=${DESTDIR:-${PWD}}

nvrc_repo="${nvrc_repo:-}"
nvrc_ref="${nvrc_ref:-}"
nvrc_toolchain="${nvrc_toolchain:-}"

[[ -n "${nvrc_repo}" ]] || nvrc_repo=$(get_from_kata_deps ".externals.nvrc.repo")
[[ -n "${nvrc_ref}" ]] || nvrc_ref=$(get_from_kata_deps ".externals.nvrc.ref")
[[ -n "${nvrc_toolchain}" ]] || nvrc_toolchain=$(get_from_kata_deps ".externals.nvrc.toolchain")

[[ -n "${nvrc_repo}" ]] || die "Failed to get nvrc repo"
[[ -n "${nvrc_ref}" ]] || die "Failed to get nvrc git ref"
[[ -n "${nvrc_toolchain}" ]] || die "Failed to get the rust toolchain to build nvrc"

container_image="${NVRC_CONTAINER_BUILDER:-$(get_nvrc_image_name)}"
# shellcheck disable=SC2154
[[ "${CROSS_BUILD}" == "true" ]] && container_image="${container_image}-cross-build"

# shellcheck disable=SC2154,SC2086
docker pull "${container_image}" || \
	(docker ${BUILDX} build ${PLATFORM} \
		--build-arg RUST_TOOLCHAIN="${nvrc_toolchain}" \
		-t "${container_image}" "${script_dir}" && \
	 # No-op unless PUSH_TO_REGISTRY is exported as "yes"
	 push_to_registry "${container_image}")

# shellcheck disable=SC2154
docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--env DESTDIR="${DESTDIR}" \
	--env nvrc_repo="${nvrc_repo}" \
	--env nvrc_ref="${nvrc_ref}" \
	--user "$(id -u)":"$(id -g)" \
	"${container_image}" \
	bash -c "${nvrc_builder}"
