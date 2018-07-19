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

# devicemapper device and options
LVM_DEVICE=${LVM_DEVICE:-/dev/vdb}
DM_STORAGE_OPTIONS="--storage-driver devicemapper --storage-opt dm.directlvm_device=${LVM_DEVICE}
	--storage-opt dm.directlvm_device_force=true --storage-opt dm.thinp_percent=95
	--storage-opt dm.thinp_metapercent=1 --storage-opt dm.thinp_autoextend_threshold=80
	--storage-opt dm.thinp_autoextend_percent=20"

# overlay storage options
OVERLAY_STORAGE_OPTIONS="--storage-driver overlay"

# Clone CRI-O repo if it is not already present.
if [ ! -d "${crio_repository_path}" ]; then
	go get -d "${crio_repository}" || true
fi

# If the change we are testing does not come from CRI-O repository,
# then checkout to the version from versions.yaml in the runtime repository.
if [ "$ghprbGhRepository" != "${crio_repository/github.com\/}" ];then
	pushd "${crio_repository_path}"
	if [ "$ID" == "fedora" ]; then
		crio_version=$(get_version "externals.crio.meta.openshift")
	else
		crio_version=$(get_version "externals.crio.version")
	fi
	git fetch
	git checkout "${crio_version}"
	popd
fi

OLD_IFS=$IFS
IFS=''

# Skip CRI-O tests that currently are not working
pushd "${crio_repository_path}/test/"
for i in "${skipCRIOTests[@]}"
do
	sed -i '/'${i}'/a skip \"This is not working (Issue https://github.com/kata-containers/agent/issues/138)\"' "$GOPATH/src/${crio_repository}/test/ctr.bats"
done

IFS=$OLD_IFS

# run CRI-O tests using devicemapper on ubuntu 16.04
MAJOR=$(echo "$VERSION_ID"|cut -d\. -f1)
if [ "$ID" == "ubuntu" ] && [ "$MAJOR" -eq 16 ]; then
	# Block device attached to the VM where we run the CI
	# If the block device has a partition, cri-o will not be able to use it.
	export LVM_DEVICE
	if sudo fdisk -l "$LVM_DEVICE" | grep "${LVM_DEVICE}[1-9]"; then
		die "detected partitions on block device: ${LVM_DEVICE}. Will not continue"
	fi
	export STORAGE_OPTIONS="$DM_STORAGE_OPTIONS"
fi

# But if on ubuntu 17.10 or newer, test using overlay
# This will allow us to run tests with at least 2 different
# storage drivers.
if [ "$ID" == "ubuntu" ] && [ "$MAJOR" -ge 17 ]; then
	export STORAGE_OPTIONS="$OVERLAY_STORAGE_OPTIONS"
fi

if [ "$ID" == "fedora" ] || [ "$ID" == "centos" ]; then
	export STORAGE_OPTIONS="$OVERLAY_STORAGE_OPTIONS"
fi


./test_runner.sh ctr.bats

popd
