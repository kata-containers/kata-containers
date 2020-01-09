#!/bin/bash
#
# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# This will enable the sandbox_cgroup_only
# to true in order to test that docker is
# working properly when this feature is
# enabled

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

dir_path=$(dirname "$0")
source "${dir_path}/../../lib/common.bash"
source "${dir_path}/../../.ci/lib.sh"
tests_repo="${tests_repo:-github.com/kata-containers/tests}"
TEST_SANDBOX_CGROUP_ONLY="${TEST_SANDBOX_CGROUP_ONLY:-}"

if [ -z "${TEST_SANDBOX_CGROUP_ONLY}" ]; then
	info "Skip: TEST_SANDBOX_CGROUP_ONLY variable is not set"
	exit 0
fi

function setup() {
	clean_env
	check_processes
}

function test_docker() {
	pushd "${GOPATH}/src/${tests_repo}"
	".ci/toggle_sandbox_cgroup_only.sh" true
	sudo -E PATH="$PATH" bash -c "make docker"
	".ci/toggle_sandbox_cgroup_only.sh" false
	popd
}

function teardown() {
	clean_env
	check_processes
}

trap teardown EXIT

echo "Running setup"
setup

echo "Running docker integration tests with sandbox cgroup enabled"
test_docker
