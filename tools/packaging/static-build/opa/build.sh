#!/usr/bin/env bash
#
# Copyright (c) 2023 Microsoft
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly opa_builder="${script_dir}/build-static-opa.sh"

source "${script_dir}/../../scripts/lib.sh"

ARCH=${ARCH:-$(uname -m)}
DESTDIR=${DESTDIR:-${PWD}}
PREFIX=${PREFIX:-/opt/kata}
container_image="${OPA_CONTAINER_BUILDER:-$(get_opa_image_name)}"
opa_repo="${opa_repo:-}"
opa_version="${opa_version:-}"

[ -n "$opa_repo" ] || opa_repo=$(get_from_kata_deps "externals.open-policy-agent.url")
[ -n "$opa_version" ] || opa_version=$(get_from_kata_deps "externals.open-policy-agent.version")

[ -n "$opa_repo" ] || die "failed to get OPA repo"
[ -n "$opa_version" ] || die "failed to get OPA version"

sudo docker pull ${container_image} || \
	(sudo docker build -t "${container_image}" "${script_dir}" && \
	# No-op unless PUSH_TO_REGISTRY is exported as "yes"
	push_to_registry "${container_image}")

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--env DESTDIR="${DESTDIR}" --env PREFIX="${PREFIX}" \
	--env opa_repo="${opa_repo}" \
	--env opa_version="${opa_version}" \
	"${container_image}" \
	bash -c "${opa_builder}"
