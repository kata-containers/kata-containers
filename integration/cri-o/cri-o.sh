#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../../.ci/lib.sh"
source "${SCRIPT_PATH}/crio_skip_tests.sh"
source /etc/os-release

crio_repository="github.com/kubernetes-incubator/cri-o"
crio_repository_path="$GOPATH/src/${crio_repository}"

# Clone CRI-O repo if it is not already present.
if [ ! -d "${crio_repository_path}" ]; then
	go get -d "${crio_repository}" || true
fi

# If the change we are testing does not come from CRI-O repository,
# then checkout to the version from versions.yaml in the runtime repository.
if [ "$ghprbGhRepository" != "${crio_repository/github.com\/}" ];then
	pushd "${crio_repository_path}"
	crio_version=$(get_version "externals.crio.version")
	git fetch
	git checkout "${crio_version}"
	popd
fi

OLD_IFS=$IFS
IFS=''

# Skip CRI-O tests that currently are not working
pushd "${crio_repository_path}/test/"
for i in ${skipCRIOTests[@]}
do
	sed -i '/'${i}'/a skip \"This is not working (Issue https://github.com/kata-containers/agent/issues/138)\"' "$GOPATH/src/${crio_repository}/test/ctr.bats"
done

IFS=$OLD_IFS

# By default run CRI-O tests using devicemapper
MAJOR=$(echo "$VERSION_ID"|cut -d\. -f1)
if [ "$ID" == "ubuntu" ]; then
	export STORAGE_OPTIONS="--storage-driver devicemapper --storage-opt dm.use_deferred_removal=false"
fi

# But if on ubuntu 17.10 or newer, test using overlay
# This will allow us to run tests with at least 2 different
# storage drivers.
if [ "$ID" == "ubuntu" ] && [ "$MAJOR" -ge 17 ]; then
	export CRIO_STORAGE_DRIVER_OPTS="--storage-driver overlay"
fi

./test_runner.sh ctr.bats

popd
