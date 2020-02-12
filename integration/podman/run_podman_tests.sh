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

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../../.ci/lib.sh"
source /etc/os-release || source /usr/lib/os-release

podman_repository="github.com/containers/libpod"
podman_repository_path="${GOPATH}/src/${podman_repository}"
# Podman configuration file
export PODMAN_CONFIG="${SCRIPT_PATH}/../../.ci/podman/configuration_podman.yaml"
# filter scheme script for podman integration test suites
export PODMAN_FILE="${SCRIPT_PATH}/../../.ci/filter/filter_podman_test.sh"
test_repository="github.com/kata-containers/tests"
test_repository_path="${GOPATH}/src/${test_repository}"
version=$(get_test_version "externals.podman.version")

function setup() {
	# Clone podman repository if it is not already present.
	if [ ! -d "${podman_repository_path}" ]; then
		go get -d "${podman_repository}" || true
	fi

	pushd "${podman_repository_path}"
	git checkout v"${version}"
	make
	popd
}

function setup_ginkgo() {
	pushd "${test_repository_path}"
	ln -sf . vendor/src
	GOPATH="${PWD}"/vendor go build ./vendor/github.com/onsi/ginkgo/ginkgo
	unlink vendor/src
	popd
}

function modify_runtime() {
	# Modify the runtime at the podman tests
	pushd "${podman_repository_path}/test/e2e"
	file="common_test.go"
	echo "Modify ${file} to use kata-runtime"
	sudo sed -i 's/ociRuntime, err = exec.LookPath("runc")/ociRuntime, err = exec.LookPath("kata-runtime")/g' "${file}"
	popd
}

function run_tests() {
	setup_ginkgo
	pushd "${podman_repository_path}/test/e2e"
	FOCUS=$(bash -c '${PODMAN_FILE} ${PODMAN_CONFIG}')
	"${test_repository_path}"/ginkgo -failFast -v --focus "${FOCUS}"
	popd
}

echo "Setup environment for podman tests"
setup
echo "Use kata-runtime for podman e2e tests"
modify_runtime
echo "Running podman e2e tests"
run_tests
