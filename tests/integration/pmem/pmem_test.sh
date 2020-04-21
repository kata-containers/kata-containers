#!/bin/bash
#
# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

dir_path=$(dirname "$0")
source "${dir_path}/../../lib/common.bash"
source "${dir_path}/../../.ci/lib.sh"
source /etc/os-release || source /usr/lib/os-release
image="fedora"
payload="tail -f /dev/null"
container_name="test-pmem"
osbuilder_repository="github.com/kata-containers/osbuilder"
osbuilder_repository_path="${GOPATH}/src/${osbuilder_repository}"
test_directory_name="test_pmem1"
test_directory=$(mktemp -d --suffix="${test_directory_name}")
device_name=""
TEST_INITRD="${TEST_INITRD:-no}"
experimental_qemu="${experimental_qemu:-false}"

if [ "$ID" == "fedora" ] || [ "$TEST_INITRD" == "yes" ] || [ "$experimental_qemu" == "true" ]; then
	issue="https://github.com/kata-containers/tests/issues/2437"
	echo "Skip pmem test ${issue}"
	exit 0
fi

function setup() {
	clean_env
	check_processes
	if [ ! -d "${osbuilder_repository_path}" ]; then
		go get -d "${osbuilder_repository}" || true
	fi
}

function test_pmem {
	# Create xfs
	sudo dd if=/dev/zero of=xfs.img bs=1M count=128
	device_name=$(sudo losetup --offset 2M --show -Pf xfs.img)
	sudo mkfs.xfs "${device_name}"

	size="2097152"
	gcc "${osbuilder_repository_path}/image-builder/nsdax.gpl.c" -o nsdax
	sudo ./nsdax "xfs.img" "${size}" "${size}"

	sudo mount "${device_name}" "${test_directory}"

	# Running container
	docker run -d --name "${container_name}" --runtime kata-runtime -v "${test_directory}:/${test_directory_name}" "${image}" sh -c "${payload}"

	# Check container
	docker exec "${container_name}" sh -exc "mount | grep ${test_directory_name} | grep '/dev/pmem' | grep 'dax'"
}

function teardown() {
	clean_env
	check_processes
	sudo umount "${test_directory}"
	sudo losetup -d "${device_name}"
	sudo rm -rf "${test_directory}"
}

trap teardown EXIT

echo "Running setup"
setup

echo "Running pmem test"
test_pmem
