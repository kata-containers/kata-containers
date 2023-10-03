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

sudo docker pull ${container_image} || \
	(sudo docker $BUILDX build $PLATFORM \
	    	--build-arg RUST_TOOLCHAIN="$(get_from_kata_deps "languages.rust.meta.newest-version")" \
		-t "${container_image}" "${script_dir}" && \
	 # No-op unless PUSH_TO_REGISTRY is exported as "yes"
	 push_to_registry "${container_image}")

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${repo_root_dir}" \
	"${container_image}" \
	bash -c "${tools_builder} ${tool}"
