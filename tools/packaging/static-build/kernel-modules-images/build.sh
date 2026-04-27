#!/usr/bin/env bash
#
# Copyright (c) 2026 Kata Contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Build kernel module disk images inside the kernel builder container.

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck disable=SC1091
source "${script_dir}/../../scripts/lib.sh"

repo_root_dir="${repo_root_dir:-}"
[[ -n "${repo_root_dir}" ]] || die "repo_root_dir is not set"

readonly modules_builder="${repo_root_dir}/tools/packaging/kernel/build-kernel-modules-images.sh"

DESTDIR=${DESTDIR:-${PWD}}
PREFIX=${PREFIX:-/opt/kata}
KBUILD_SIGN_PIN="${KBUILD_SIGN_PIN:-}"

output_dir="${DESTDIR}/${PREFIX}/share/kata-containers"
mkdir -p "${output_dir}"

container_image="${KERNEL_CONTAINER_BUILDER:-$(get_kernel_image_name)}"
container_engine="${CONTAINER_ENGINE:-docker}"

"${container_engine}" pull "${container_image}" || \
	"${container_engine}" build \
		--build-arg "ARCH=${ARCH:-}" \
		-t "${container_image}" \
		"${script_dir}/../kernel"

"${container_engine}" run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	--privileged \
	-w "${PWD}" \
	--env KBUILD_SIGN_PIN="${KBUILD_SIGN_PIN}" \
	"${container_image}" \
	bash -c "${modules_builder} -a ${ARCH:-$(uname -m)} -o ${output_dir}"
