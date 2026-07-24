#!/usr/bin/env bash
#
# Copyright (c) 2026 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly openvmm_builder="${script_dir}/build-static-openvmm.sh"

# shellcheck source=/dev/null
source "${script_dir}/../../scripts/lib.sh"

ARCH=${ARCH:-$(uname -m)}
openvmm_repo="${openvmm_repo:-}"
openvmm_version="${openvmm_version:-}"

[[ -n "${openvmm_repo}" ]] || openvmm_repo=$(get_from_kata_deps ".assets.hypervisor.openvmm.url")
[[ -n "${openvmm_version}" ]] || openvmm_version=$(get_from_kata_deps ".assets.hypervisor.openvmm.version")

[[ -n "${openvmm_repo}" ]] || die "Failed to get openvmm repo"
[[ -n "${openvmm_version}" ]] || die "Failed to get openvmm version"

container_image="${OPENVMM_CONTAINER_BUILDER:-$(get_openvmm_image_name)}"

docker pull "${container_image}" || \
	(docker build \
		-t "${container_image}" "${script_dir}" && \
	 # No-op unless PUSH_TO_REGISTRY is exported as "yes"
	 push_to_registry "${container_image}")

# shellcheck disable=SC2154
docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--env openvmm_repo="${openvmm_repo}" \
	--env openvmm_version="${openvmm_version}" \
	--env ARCH="${ARCH}" \
	--env HOST_UID="$(id -u)" \
	--env HOST_GID="$(id -g)" \
	"${container_image}" \
	bash -c "${openvmm_builder}"
