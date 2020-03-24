#!/bin/bash
#
# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# This will test the default_vcpus
# feature is working properly

[ -n "$DEBUG" ] && set -x
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

dir_path=$(dirname "$0")
source "${dir_path}/../../lib/common.bash"
source "${dir_path}/../../.ci/lib.sh"
KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"
name="${name:-default_vcpus}"

function setup() {
	clean_env
	check_processes
	extract_kata_env
	sudo sed -i "s/${name} = 1/${name} = 4/g" "${RUNTIME_CONFIG_PATH}"
}

function test_docker_with_vcpus() {
	docker run --runtime="${RUNTIME}" busybox nproc | grep "4"
}

function teardown() {
	sudo sed -i "s/${name} = 4/${name} = 1/g" "${RUNTIME_CONFIG_PATH}"
	clean_env
	check_processes
}

trap teardown EXIT

echo "Running setup"
setup

echo "Running docker integration tests with vcpus"
test_docker_with_vcpus
