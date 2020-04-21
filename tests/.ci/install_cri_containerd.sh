#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

source /etc/os-release || source /usr/lib/os-release

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cri_repository="github.com/containerd/cri"

# Flag to do tasks for CI
CI=${CI:-""}

# shellcheck source=./lib.sh
source "${script_dir}/lib.sh"

#Use cri contaienrd tarball format.
#https://github.com/containerd/cri/blob/master/docs/installation.md#release-tarball
CONTAINERD_OS=$(go env GOOS)
CONTAIENRD_ARCH=$(go env GOARCH)

cri_containerd_tarball_version=$(get_version "externals.cri-containerd.version")
cri_containerd_repo=$(get_version "externals.cri-containerd.url")

echo "Get cri_containerd version"
cri_containerd_version_url="https://raw.githubusercontent.com/containerd/containerd/${cri_containerd_tarball_version}/vendor.conf"
cri_containerd_version=$(curl -sL $cri_containerd_version_url | grep "github.com/containerd/cri" | awk '{print $2}')

echo "Set up environment"
if [ "$ID" == centos ]; then
	# Centos: remove seccomp  from runc build
	export BUILDTAGS=${BUILDTAGS:-apparmor}
fi

install_from_source() {
	echo "Trying to install containerd from source"
	(
		cd "${GOPATH}/src/${cri_repository}" >>/dev/null
		git fetch
		git checkout "${cri_containerd_version}"
		make release
		local commit
		commit=$(git rev-parse --short HEAD)
		tarball_name="cri-containerd-${commit}.${CONTAINERD_OS}-${CONTAIENRD_ARCH}.tar.gz"
		sudo tar -xvf "./_output/${tarball_name}" -C /
	)
}

install_from_static_tarball() {
	echo "Trying to install containerd from static tarball"
	local tarball_url=$(get_version "externals.cri-containerd.tarball_url")
	cri_containerd_tarball_version="${cri_containerd_tarball_version/v/}"

	local tarball_name="cri-containerd-${cri_containerd_tarball_version}.${CONTAINERD_OS}-${CONTAIENRD_ARCH}.tar.gz"
	local url="${tarball_url}/${tarball_name}"

	echo "Download tarball from ${url}"
	if ! curl -OL -f "${url}"; then
		echo "Failed to download tarball from ${url}"
		return 1
	fi

	sudo tar -xvf "${tarball_name}" -C /
	echo "vendored cri version ${cri_containerd_version}"
	(
		cd "${GOPATH}/src/${cri_containerd_repo}" >>/dev/null
		echo "checkout to cri version ${cri_containerd_version}"
		git checkout "${cri_containerd_version}"
	)
}

go get "${cri_containerd_repo}"
install_from_static_tarball || install_from_source

sudo systemctl daemon-reload
