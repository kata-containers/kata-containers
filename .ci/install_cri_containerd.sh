#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cri_repository="github.com/containerd/cri"

# Flag to do tasks for CI
CI=${CI:-""}

source "${script_dir}/lib.sh"
cri_containerd_version=$(get_version "externals.cri-containerd.version")
containerd_version=$(get_version "externals.cri-containerd.meta.containerd-version")

source /etc/os-release || source /usr/lib/os-release

echo "Set up environment"
if [ "$ID" == centos ];then
	# Centos: remove seccomp  from runc build
	export BUILDTAGS=${BUILDTAGS:-apparmor}
fi

go get github.com/containerd/cri
pushd "${GOPATH}/src/${cri_repository}" >> /dev/null
git fetch
git checkout "${cri_containerd_version}"
make
sudo -E PATH=$PATH make install.deps
sudo -E PATH=$PATH make install
if [ -n "$CI" ]; then
	cni_test_dir="/etc/cni-containerd-test"
	sudo mkdir -p "${cni_test_dir}"
	# if running on CI use a different CNI directory (cri-o and kubernetes configurations may be installed)
	sudo mv /etc/cni/net.d/10-containerd-net.conflist  "$cni_test_dir"
fi

popd  >> /dev/null
