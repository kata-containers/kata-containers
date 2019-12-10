#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

readonly script_dir=$(dirname $(readlink -f "$0"))

cidir=$(dirname "$0")
arch=$("${cidir}"/kata-arch.sh -d)
source "${cidir}/lib.sh"
# Where real kata build script exist, via docker build to avoid install all deps
packaging_repo="github.com/kata-containers/packaging"
latest_build_url="${jenkins_url}/job/cloud-hypervisor-nightly-$(uname -m)/${cached_artifacts_path}"


install_clh() {
	# Get url for cloud_hypervisor from runtime/versions.yaml
	cloud_hypervisor_repo=$(get_version "assets.hypervisor.cloud_hypervisor.url")
	[ -n "$cloud_hypervisor_repo" ] || die "failed to get cloud_hypervisor repo"
	export cloud_hypervisor_repo
	go_cloud_hypervisor_repo=${cloud_hypervisor_repo/https:\/\//}

	# Get version for cloud_hypervisor from runtime/versions.yaml
	cloud_hypervisor_version=$(get_version "assets.hypervisor.cloud_hypervisor.version")
	[ -n "$cloud_hypervisor_version" ] || die "failed to get cloud_hypervisor version"
	export cloud_hypervisor_version

	# Get cloud_hypervisor repo
	go get -d "${go_cloud_hypervisor_repo}" || true
	# This may be downloaded before if there was a depends-on in PR, but 'go get' wont make any problem here
	go get -d "${packaging_repo}" || true
	pushd "${GOPATH}/src/${go_cloud_hypervisor_repo}"
	# packaging build script expects run in the hypervisor repo parent directory
	# It will find the hypervisor repo and checkout to the version exported above
	${GOPATH}/src/${packaging_repo}/static-build/cloud-hypervisor/build-static-clh.sh
	sudo install -D cloud-hypervisor/cloud-hypervisor /usr/bin/cloud-hypervisor
	popd
}

install_cached_clh(){
	local cloud_hypervisor_binary=${1:-}
	[ -z "${cloud_hypervisor_binary}" ] && die "empty binary format"
	info "Installing ${cloud_hypervisor_binary}"
	local cloud_hypervisor_path="/usr/bin"
	local cloud_hypervisor_name="cloud-hypervisor"
	sudo -E curl -fL --progress-bar "${latest_build_url}"/"${cloud_hypervisor_binary_name}" -o "${cloud_hypervisor_binary_path}" || return 1
}

install_prebuilt_clh() {
	local cloud_hypervisor_path="/usr/bin"
	pushd "${cloud_hypervisor_path}" > /dev/null
	info "Verify download checksum"
	sudo -E curl -fsOL "${latest_build_url}/sha256sum-cloud-hypervisor" || return 1
	sudo sha256sum -c "sha256sum-cloud-hypervisor" || return 1
	popd > /dev/null
}

main() {
	current_cloud_hypervisor_version=$(get_version "assets.hypervisor.cloud_hypervisor.version")
	cached_cloud_hypervisor_version=$(curl -sfL "${latest_build_url}/latest") || cached_cloud_hypervisor_version="none"
	info "current cloud hypervisor : ${current_cloud_hypervisor_version}"
	info "cached cloud hypervisor  : ${cached_cloud_hypervisor_version}"
	if [ "$cached_cloud_hypervisor_version" == "$current_cloud_hypervisor_version" ] && [ "$arch" == "x86_64" ]; then
		if ! install_prebuilt_clh; then
			info "failed to install cached cloud hypervisor, trying to build from source"
			install_clh
		fi
	else
		install_clh
	fi
}

main "$@"
