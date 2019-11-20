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
# Where real kata build script exist, via docker build to avoid install all deps
packaging_repo="github.com/kata-containers/packaging"

source "${cidir}/lib.sh"


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

main() {
	install_clh
}

main "$@"
