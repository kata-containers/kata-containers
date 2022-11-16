#!/usr/bin/env bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

ARCH=$(uname -m)

# Currently, Cloud Hypervisor only support arm64 and x86_64
[ "${ARCH}" != "aarch64" ] && [ "${ARCH}" != "x86_64" ] && exit

script_dir=$(dirname $(readlink -f "$0"))
force_build_from_source="${force_build_from_source:-false}"
features="${features:-}"

source "${script_dir}/../../scripts/lib.sh"

cloud_hypervisor_repo="${cloud_hypervisor_repo:-}"
cloud_hypervisor_version="${cloud_hypervisor_version:-}"
cloud_hypervisor_pr="${cloud_hypervisor_pr:-}"
cloud_hypervisor_pull_ref_branch="${cloud_hypervisor_pull_ref_branch:-main}"
cloud_hypervisor_latest_build_url="${jenkins_url}/job/kata-containers-2.0-clh-cc-$(uname -m)/${cached_artifacts_path}"

if [ -z "$cloud_hypervisor_repo" ]; then
	info "Get cloud_hypervisor information from runtime versions.yaml"
	cloud_hypervisor_url=$(get_from_kata_deps "assets.hypervisor.cloud_hypervisor.url")
	[ -n "$cloud_hypervisor_url" ] || die "failed to get cloud_hypervisor url"
	cloud_hypervisor_repo="${cloud_hypervisor_url}.git"
fi
[ -n "$cloud_hypervisor_repo" ] || die "failed to get cloud_hypervisor repo"

if [ -n "$cloud_hypervisor_pr" ]; then
	force_build_from_source=true
	cloud_hypervisor_version="PR $cloud_hypervisor_pr"
else
	[ -n "$cloud_hypervisor_version" ] || cloud_hypervisor_version=$(get_from_kata_deps "assets.hypervisor.cloud_hypervisor.version")
	[ -n "$cloud_hypervisor_version" ] || die "failed to get cloud_hypervisor version"
fi

pull_clh_released_binary() {
	info "Download cloud-hypervisor version: ${cloud_hypervisor_version}"
	cloud_hypervisor_binary="https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/${cloud_hypervisor_version}/cloud-hypervisor-static"

	curl --fail -L ${cloud_hypervisor_binary} -o cloud-hypervisor-static || return 1
	mkdir -p cloud-hypervisor
	mv -f cloud-hypervisor-static cloud-hypervisor/cloud-hypervisor
	chmod +x cloud-hypervisor/cloud-hypervisor
}

build_clh_from_source() {
	info "Build ${cloud_hypervisor_repo} version: ${cloud_hypervisor_version}"
	repo_dir=$(basename "${cloud_hypervisor_repo}")
	repo_dir="${repo_dir//.git}"
	rm -rf "${repo_dir}"
	git clone "${cloud_hypervisor_repo}"
	git config --global --add safe.directory "$PWD/repo_dir"
	pushd "${repo_dir}"

	if [ -n "${cloud_hypervisor_pr}" ]; then
		local pr_branch="PR_${cloud_hypervisor_pr}"
		git fetch origin "pull/${cloud_hypervisor_pr}/head:${pr_branch}" || return 1
		git checkout "${pr_branch}"
		git rebase "origin/${cloud_hypervisor_pull_ref_branch}"

		git log --oneline main~1..HEAD
	else
		git fetch || true
		git checkout "${cloud_hypervisor_version}"
	fi

	if [ -n "${features}" ]; then
		info "Build cloud-hypervisor enabling the following features: ${features}"
		./scripts/dev_cli.sh build --release --libc musl --features "${features}"
	else
		./scripts/dev_cli.sh build --release --libc musl
	fi
	rm -f cloud-hypervisor
	cp build/cargo_target/$(uname -m)-unknown-linux-musl/release/cloud-hypervisor .
	popd
}

check_cached_cloud_hypervisor() {
	local cached_cloud_hypervisor_version=$(curl -sfL "${cloud_hypervisor_latest_build_url}"/latest) || latest="none"
	info "Current cloud hypervisor version: ${cloud_hypervisor_version}"
	info "Cached cloud hypervisor version: ${cached_cloud_hypervisor_version}"
	if [ "${cloud_hypervisor_version}" == "${cached_cloud_hypervisor_version}" ] && [ "${ARCH}" == "x86_64" ]; then
		install_cached_cloud_hypervisor
	else
		build_clh_from_source
	fi
}

install_cached_cloud_hypervisor() {
	local cached_path="$(echo ${script_dir} | sed 's,/*[^/]\+/*$,,' | sed 's,/*[^/]\+/*$,,' | sed 's,/*[^/]\+/*$,,' | sed 's,/*[^/]\+/*$,,')"
	local clh_directory="${cached_path}/tools/packaging/kata-deploy/local-build/build/cc-cloud-hypervisor/builddir/cloud-hypervisor"
	mkdir cloud-hypervisor
	pushd cloud-hypervisor
	local checksum_file="sha256sum-cloud-hypervisor"
	info "Downloading the cloud hypervisor binary"
	curl -fOL --progress-bar "${cloud_hypervisor_latest_build_url}/cloud-hypervisor" || return 1
	info "Checking cloud hypervisor binary checksum"
	curl -fOL --progress-bar "${cloud_hypervisor_latest_build_url}/${checksum_file}" || return 1
	info "Verify checksum"
	sudo sha256sum -c "${checksum_file}" || return 1
	chmod +x cloud-hypervisor
	local clh_binary_path="${cached_path}/cloud-hypervisor"
	if [ ! -d "${clh_binary_path}" ]; then
		mkdir -p "${clh_binary_path}"
	fi
	if [ ! -f "${clh_binary_path}/cloud-hypervisor" ]; then
		cp cloud-hypervisor "${clh_binary_path}"
	fi
	popd
}

if [ "${ARCH}" == "aarch64" ]; then
	info "aarch64 binaries are not distributed as part of the Cloud Hypervisor releases, forcing to build from source"
	force_build_from_source="true"
fi

if [ -n "${features}" ]; then
	info "As an extra build argument has been passed to the script, forcing to build from source"
	force_build_from_source="true"
fi

if [ "${force_build_from_source}" == "true" ]; then
	info "Build cloud-hypervisor from source as it's been request via the force_build_from_source flag"
	check_cached_cloud_hypervisor
else
	pull_clh_released_binary ||
	(info "Failed to pull cloud-hypervisor released binary, trying to build from source" && check_cached_cloud_hypervisor)
fi
