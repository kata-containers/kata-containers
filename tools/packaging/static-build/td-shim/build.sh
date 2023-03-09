#!/usr/bin/env bash
#
# Copyright (c) 2022 Intel
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly tdshim_builder="${script_dir}/build-td-shim.sh"

source "${script_dir}/../../scripts/lib.sh"

DESTDIR=${DESTDIR:-${PWD}}
PREFIX=${PREFIX:-/opt/kata}
kata_version="${kata_version:-}"
tdshim_repo="${tdshim_repo:-}"
tdshim_version="${tdshim_version:-}"
tdshim_toolchain="${tdshim_toolchain:-}"
package_output_dir="${package_output_dir:-}"

[ -n "${tdshim_repo}" ] || tdshim_repo=$(get_from_kata_deps "externals.td-shim.url" "${kata_version}")
[ -n "${tdshim_version}" ] || tdshim_version=$(get_from_kata_deps "externals.td-shim.version" "${kata_version}")
[ -n "${tdshim_toolchain}" ] || tdshim_toolchain=$(get_from_kata_deps "externals.td-shim.toolchain" "${kata_version}")

[ -n "${tdshim_repo}" ] || die "Failed to get TD-shim repo"
[ -n "${tdshim_version}" ] || die "Failed to get TD-shim version or commit"
[ -n "${tdshim_toolchain}" ] || die "Failed to get TD-shim toolchain to be used to build the project"

container_image="${TDSHIM_CONTAINER_BUILDER:-${BUILDER_REGISTRY}:td-shim-${tdshim_toolchain}-$(get_last_modification ${script_dir})-$(uname -m)}"

sudo docker pull ${container_image} || (sudo docker build \
	--build-arg RUST_TOOLCHAIN="${tdshim_toolchain}" \
	-t "${container_image}" \
	"${script_dir}" && \
	# No-op unless PUSH_TO_REGISTRY is exported as "yes"
	push_to_registry "${container_image}")

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--env DESTDIR="${DESTDIR}" \
	--env PREFIX="${PREFIX}" \
	--env tdshim_repo="${tdshim_repo}" \
	--env tdshim_version="${tdshim_version}" \
	"${container_image}" \
	bash -c "${tdshim_builder}"
