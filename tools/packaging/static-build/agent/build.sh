#!/usr/bin/env bash
#
# Copyright (c) 2023 Intel
#
# SPDX-License-Identifier: Apache-2.0

set -x
set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly agent_builder="${script_dir}/build-static-agent.sh"

source "${script_dir}/../../scripts/lib.sh"

container_image="${AGENT_CONTAINER_BUILDER:-$(get_agent_image_name)}"
[ "${CROSS_BUILD}" == "true" ] && container_image="${container_image}-cross-build"

sudo docker pull ${container_image} || \
	(sudo docker $BUILDX build $PLATFORM \
	    	--build-arg RUST_TOOLCHAIN="$(get_from_kata_deps "languages.rust.meta.newest-version")" \
            --build-arg LIBSECCOMP_LIB_PATH="${LIBSECCOMP_LIB_PATH:-/usr/lib}" \
		-t "${container_image}" "${script_dir}" && \
	 # No-op unless PUSH_TO_REGISTRY is exported as "yes"
	 push_to_registry "${container_image}")

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	--env DESTDIR=${DESTDIR} \
	--env AGENT_POLICY=${AGENT_POLICY:-no} \
    --env LIBSECCOMP_LIB_PATH="${LIBSECCOMP_LIB_PATH:-/usr/lib}" \
    --env LIBCLANG_PATH="${LIBCLANG_PATH:-/usr/lib/llvm-14/lib/libclang.so}" \
	-w "${repo_root_dir}" \
	"${container_image}" \
	bash -c "${agent_builder}"