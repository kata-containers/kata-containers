#!/usr/bin/env bash
#
# Copyright (c) 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "${script_dir}/../../scripts/lib.sh"

export qemu_repo="${qemu_repo:-}"
export qemu_version="${qemu_version:-}"
export qemu_latest_build_url="${jenkins_url}/job/kata-containers-2.0-qemu-cc-$(uname -m)/${cached_artifacts_path}"
export katacontainers_repo="${katacontainers_repo:=github.com/kata-containers/kata-containers}"
export qemu_tarball_name="kata-static-qemu-cc.tar.gz"
export pkg_dir="$(echo $script_dir | sed 's,/*[^/]\+/*$,,' | sed 's,/*[^/]\+/*$,,')"
export qemu_tarball_directory="${pkg_dir}/kata-deploy/local-build/build/cc-qemu/builddir"
export tee="${tee:-}"

export prefix="/opt/confidential-containers/"

get_qemu_information() {
	if [ -z "${qemu_repo}" ]; then
		info "Get qemu information from runtime versions.yaml"
		export qemu_url=$(get_from_kata_deps "assets.hypervisor.qemu.url")
		[ -n "${qemu_url}" ] || die "failed to get qemu url"
		export qemu_repo="${qemu_url}.git"
	fi

	[ -n "${qemu_repo}" ] || die "failed to get qemu repo"
	[ -n "${qemu_version}" ] || export qemu_version=$(get_from_kata_deps "assets.hypervisor.qemu.version")
	[ -n "${qemu_version}" ] || die "failed to get qemu version"
}

cached_or_build_qemu_tar() {
	# Check latest qemu cc tar version sha256sum
	local latest=$(curl -sfL "${qemu_latest_build_url}/latest") || latest="none"
	local cached_qemu_version="$(echo ${latest} | awk '{print $1}')"
	info "Current qemu version: ${qemu_version}"
	info "Cached qemu version: ${cached_qemu_version}"
	if [ "${qemu_version}" == "${cached_qemu_version}" ]; then
		info "Get latest cached information ${latest}"
		local cached_sha256sum="$(echo ${latest} | awk '{print $2}')"
		info "Cached sha256sum version: ${cached_sha256sum}"
		local current_sha256sum="$(calc_qemu_files_sha256sum)"
		info "Current sha256sum of the qemu directory ${current_sha256sum}"
		if [ -z "${cached_sha256sum}" ]; then
			build_qemu_tar
		elif [ "${current_sha256sum}" == "${cached_sha256sum}" ]; then
			install_cached_qemu_tar
		else
			build_qemu_tar
		fi
	else
		build_qemu_tar
	fi
}

build_qemu_tar() {
	[ -n "${tee}" ] && qemu_tarball_name="kata-static-${tee}-qemu-cc.tar.gz"
	"${script_dir}/build-base-qemu.sh" "${qemu_repo}" "${qemu_version}" "${tee}" "${qemu_tarball_name}"
}

install_cached_qemu_tar() {
	info "Using cached tarball of qemu"
	curl -fL --progress-bar "${qemu_latest_build_url}/${qemu_tarball_name}" -o "${qemu_tarball_name}" || return 1
	curl -fsOL "${qemu_latest_build_url}/sha256sum-${qemu_tarball_name}" || return 1
	sha256sum -c "sha256sum-${qemu_tarball_name}" || return 1
}

main() {
	get_qemu_information
	# Currently the cached for qemu cc only works in x86_64
	if [ "$(uname -m)" == "x86_64" ]; then
		cached_or_build_qemu_tar
	else
		build_qemu_tar
	fi
}

main $@
