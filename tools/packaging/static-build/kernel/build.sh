#!/usr/bin/env bash
#
# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly repo_root_dir="$(cd "${script_dir}/../../../.." && pwd)"
readonly kernel_builder="${repo_root_dir}/tools/packaging/kernel/build-kernel.sh"

source "${script_dir}/../../scripts/lib.sh"

DESTDIR=${DESTDIR:-${PWD}}
PREFIX=${PREFIX:-/opt/kata}
container_image="${KERNEL_CONTAINER_BUILDER:-${CC_BUILDER_REGISTRY}:kernel-$(get_last_modification ${repo_root_dir} ${script_dir})-$(uname -m)}"
kernel_latest_build_url="${jenkins_url}/job/kata-containers-2.0-kernel-cc-$(uname -m)/${cached_artifacts_path}"
current_kernel_version=${kernel_version:-$(get_from_kata_deps "assets.kernel.version")}
cached_path="$(echo ${script_dir} | sed 's,/*[^/]\+/*$,,' | sed 's,/*[^/]\+/*$,,' | sed 's,/*[^/]\+/*$,,' | sed 's,/*[^/]\+/*$,,')"

build_from_source() {
	sudo docker pull ${container_image} || \
		(sudo docker build -t "${container_image}" "${script_dir}" && \
	 	# No-op unless PUSH_TO_REGISTRY is exported as "yes"
	 	push_to_registry "${container_image}")

	sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
		-w "${PWD}" \
		--env KATA_BUILD_CC="${KATA_BUILD_CC:-}" \
		"${container_image}" \
		bash -c "${kernel_builder} $* setup"

	sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
		-w "${PWD}" \
		"${container_image}" \
		bash -c "${kernel_builder} $* build"

	sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
		-w "${PWD}" \
		--env DESTDIR="${DESTDIR}" --env PREFIX="${PREFIX}" \
		"${container_image}" \
		bash -c "${kernel_builder} $* install"
}

check_cached_kernel() {
	local latest=$(curl -sfL "${kernel_latest_build_url}"/latest) || latest="none"
	local cached_kernel_version="$(echo ${latest} | awk '{print $1}')"
	info "Current kernel version: ${current_kernel_version}"
	info "Cached kernel version: ${cached_kernel_version}"
	if [ "${current_kernel_version}" == "${cached_kernel_version}" ] && [ "${ARCH}" == "x86_64" ]; then
		local cached_kernel_config="$(echo ${latest} | awk '{print $2}')"
		info "Cached kernel config: ${cached_kernel_config}"
		local current_kernel_config_file="${cached_path}/tools/packaging/kernel/kata_config_version"
		local current_kernel_config="$(cat $current_kernel_config_file)"
		info "Current kernel config: ${current_kernel_config}"
		if [ -z "${cached_kernel_config}" ]; then
			build_from_source
		else
			install_cached_kernel
		fi
	else
		build_from_source
	fi
}

install_cached_kernel() {
	local kernel_directory="${cached_path}/tools/packaging/kata-deploy/local-build/build/cc-kernel/builddir"
        local kernel_tarball_name="linux-${cached_kernel_version}.tar.xz"
	mkdir -p "${kernel_directory}"
	pushd "${kernel_directory}"
	curl -fL --progress-bar "${kernel_latest_build_url}/${kernel_tarball_name}" -o "${kernel_tarball_name}" || return 1
	popd

}

main() {
	check_cached_kernel
}

main "$@"
