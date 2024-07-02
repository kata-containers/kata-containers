#!/usr/bin/env bash
#
# Copyright (c) 2024 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

set -x

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
source "${script_dir}/../../scripts/lib.sh"


readonly busybox_builder="${script_dir}/build-static-busybox.sh"

busybox_version="$(get_from_kata_deps ".externals.busybox.version")"
readonly BUSYBOX_VERSION=${busybox_version}

busybox_url="$(get_from_kata_deps ".externals.busybox.url")"
readonly BUSYBOX_URL="${busybox_url}"


container_image="${BUSYBOX_CONTAINER_BUILDER:-$(get_busybox_image_name)}"
[ "${CROSS_BUILD}" == "true" ] && container_image="${container_image}-cross-build"

docker pull "${container_image}" || \
	(docker $BUILDX build $PLATFORM \
		-t "${container_image}" "${script_dir}" \
	 # No-op unless PUSH_TO_REGISTRY is exported as "yes"
	 push_to_registry "${container_image}")

docker run --rm -i -v "${repo_root_dir:?}:${repo_root_dir}" \
	--env DESTDIR="${DESTDIR:?}" \
	--env BUSYBOX_VERSION="${BUSYBOX_VERSION:?}" \
	--env BUSYBOX_URL="${BUSYBOX_URL:?}" \
	--env BUSYBOX_CONF_FILE="${BUSYBOX_CONF_FILE:?}" \
	--env BUSYBOX_CONF_DIR="${script_dir:?}" \
	--env HOME="/tmp" \
	--user "$(id -u):$(id -g)" \
	-w "${repo_root_dir}/build/busybox/builddir" \
	"${container_image}" \
	sh -c "${busybox_builder}"
