#!/usr/bin/env bash
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Build the selinux guest extension tree (binary policy, file contexts and the
# per-extension mount contexts NVRC consumes) into ${DESTDIR}.

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
source "${script_dir}/../../scripts/lib.sh"

container_image="${GUEST_SELINUX_CONTAINER_BUILDER:-$(get_guest_selinux_image_name)}"

# shellcheck disable=SC2154,SC2086
docker pull "${container_image}" || \
	(docker build \
		-t "${container_image}" "${script_dir}" && \
	 # No-op unless PUSH_TO_REGISTRY is exported as "yes"
	 push_to_registry "${container_image}")

mkdir -p "${DESTDIR:?}"
# The builder rewrites its own policy store, so it runs as root; hand the
# output back to the calling user afterwards.
# shellcheck disable=SC2154
docker run --rm -i -v "${repo_root_dir:?}:${repo_root_dir}" \
	-v "${DESTDIR}:${DESTDIR}" \
	--env DESTDIR="${DESTDIR}" \
	--env GUEST_SELINUX_POLICY_VERSION="${GUEST_SELINUX_POLICY_VERSION:-35}" \
	"${container_image}" \
	bash -c "${script_dir}/build-guest-selinux.sh && chown -R $(id -u):$(id -g) ${DESTDIR}"
