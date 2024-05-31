#!/usr/bin/env bash
#
# Copyright (c) 2022 IBM
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly ovmf_builder="${script_dir}/build-ovmf.sh"

source "${script_dir}/../../scripts/lib.sh"

DESTDIR=${DESTDIR:-${PWD}}
PREFIX=${PREFIX:-/opt/kata}
container_image="${OVMF_CONTAINER_BUILDER:-$(get_ovmf_image_name)}"
ovmf_build="${ovmf_build:-x86_64}"
kata_version="${kata_version:-}"
ovmf_repo="${ovmf_repo:-}"
ovmf_version="${ovmf_version:-}"
ovmf_package="${ovmf_package:-}"
package_output_dir="${package_output_dir:-}"

if [ -z "$ovmf_repo" ]; then
	ovmf_repo=$(get_from_kata_deps ".externals.ovmf.url")
fi

[ -n "$ovmf_repo" ] || die "failed to get ovmf repo"

if [ "${ovmf_build}" == "x86_64" ]; then
	[ -n "$ovmf_version" ] || ovmf_version=$(get_from_kata_deps ".externals.ovmf.x86_64.version")
	[ -n "$ovmf_package" ] || ovmf_package=$(get_from_kata_deps ".externals.ovmf.x86_64.package")
	[ -n "$package_output_dir" ] || package_output_dir=$(get_from_kata_deps ".externals.ovmf.x86_64.package_output_dir")
elif [ "${ovmf_build}" == "sev" ]; then
	[ -n "$ovmf_version" ] || ovmf_version=$(get_from_kata_deps ".externals.ovmf.sev.version")
	[ -n "$ovmf_package" ] || ovmf_package=$(get_from_kata_deps ".externals.ovmf.sev.package")
	[ -n "$package_output_dir" ] || package_output_dir=$(get_from_kata_deps ".externals.ovmf.sev.package_output_dir")
elif [ "${ovmf_build}" == "tdx" ]; then
	[ -n "$ovmf_version" ] || ovmf_version=$(get_from_kata_deps ".externals.ovmf.tdx.version")
	[ -n "$ovmf_package" ] || ovmf_package=$(get_from_kata_deps ".externals.ovmf.tdx.package")
	[ -n "$package_output_dir" ] || package_output_dir=$(get_from_kata_deps ".externals.ovmf.tdx.package_output_dir")
fi

[ -n "$ovmf_version" ] || die "failed to get ovmf version or commit"
[ -n "$ovmf_package" ] || die "failed to get ovmf package or commit"
[ -n "$package_output_dir" ] || die "failed to get ovmf package or commit"

docker pull ${container_image} || \
	(docker build -t "${container_image}" "${script_dir}" && \
	# No-op unless PUSH_TO_REGISTRY is exported as "yes"
	push_to_registry "${container_image}")

docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--env DESTDIR="${DESTDIR}" --env PREFIX="${PREFIX}" \
	--env ovmf_build="${ovmf_build}" \
	--env ovmf_repo="${ovmf_repo}" \
	--env ovmf_version="${ovmf_version}" \
	--env ovmf_package="${ovmf_package}" \
	--env package_output_dir="${package_output_dir}" \
	--user "$(id -u)":"$(id -g)" \
	"${container_image}" \
	bash -c "${ovmf_builder}"
