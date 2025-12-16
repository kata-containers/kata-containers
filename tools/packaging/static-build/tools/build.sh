#!/usr/bin/env bash
#
# Copyright (c) 2023 Intel
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly tools_builder="${script_dir}/build-static-tools.sh"

source "${script_dir}/../../scripts/lib.sh"

tool="${1}"

container_image="${TOOLS_CONTAINER_BUILDER:-$(get_tools_image_name)}"
[ "${CROSS_BUILD}" == "true" ] && container_image="${container_image}-cross-build"

docker pull ${container_image} || \
	(docker $BUILDX build $PLATFORM \
	    	--build-arg GO_TOOLCHAIN="$(get_from_kata_deps ".languages.golang.meta.newest-version")" \
	    	--build-arg RUST_TOOLCHAIN="$(get_from_kata_deps ".languages.rust.meta.newest-version")" \
		-t "${container_image}" "${script_dir}" && \
	 # No-op unless PUSH_TO_REGISTRY is exported as "yes"
	 push_to_registry "${container_image}")

docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	--env LIBSECCOMP_VERSION=${LIBSECCOMP_VERSION} \
	--env LIBSECCOMP_URL=${LIBSECCOMP_URL} \
	--env GPERF_VERSION=${GPERF_VERSION} \
	--env GPERF_URL=${GPERF_URL} \
	--env ORAS_CACHE_HELPER="${repo_root_dir}/tools/packaging/scripts/download-with-oras-cache.sh" \
	--env USE_ORAS_CACHE="${USE_ORAS_CACHE:-yes}" \
	--env PUSH_TO_REGISTRY="${PUSH_TO_REGISTRY:-no}" \
	--env GH_TOKEN="${GH_TOKEN:-}" \
	--env GITHUB_ACTOR="${GITHUB_ACTOR:-}" \
	-w "${repo_root_dir}" \
	--user "$(id -u)":"$(id -g)" \
	"${container_image}" \
	bash -c "${tools_builder} ${tool}"
