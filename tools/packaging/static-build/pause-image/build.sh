#!/usr/bin/env bash
#
# Copyright (c) 2024 Intel
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly pause_image_builder="${script_dir}/build-static-pause-image.sh"

source "${script_dir}/../../scripts/lib.sh"

DESTDIR=${DESTDIR:-${PWD}}

pause_image_repo="${pause_image_repo:-}"
pause_image_version="${pause_image_version:-}"
package_output_dir="${package_output_dir:-}"

[ -n "${pause_image_repo}" ] || pause_image_repo=$(get_from_kata_deps ".externals.pause.repo")
[ -n "${pause_image_version}" ] || pause_image_version=$(get_from_kata_deps ".externals.pause.version")

[ -n "${pause_image_repo}" ] || die "Failed to get pause image repo"
[ -n "${pause_image_version}" ] || die "Failed to get pause image version or commit"

container_image="${PAUSE_IMAGE_CONTAINER_BUILDER:-$(get_pause_image_name)}"
[ "${CROSS_BUILD}" == "true" ] && container_image="${container_image}-cross-build"

docker pull ${container_image} || \
	(docker $BUILDX build $PLATFORM \
		-t "${container_image}" "${script_dir}" && \
	 # No-op unless PUSH_TO_REGISTRY is exported as "yes"
	 push_to_registry "${container_image}")

docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--env DESTDIR="${DESTDIR}" \
	--env pause_image_repo="${pause_image_repo}" \
	--env pause_image_version="${pause_image_version}" \
	--user "$(id -u)":"$(id -g)" \
	"${container_image}" \
	bash -c "${pause_image_builder}"
