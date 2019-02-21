#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

source /etc/os-release

echo "Setup script for packaging"

if [ "$ID" == ubuntu ]; then
	echo "Install snap dependencies"
	sudo apt-get install -y snapd snapcraft

	echo "Install kernel dependencies"
	sudo -E apt install -y libelf-dev bc gcc
fi

export tests_repo="${tests_repo:-github.com/kata-containers/tests}"
export tests_repo_dir="$GOPATH/src/$tests_repo"

clone_tests_repo()
{
	# KATA_CI_NO_NETWORK is (has to be) ignored if there is
	# no existing clone.
	if [ -d "$tests_repo_dir" -a -n "${KATA_CI_NO_NETWORK:-}" ]
	then
		return
	fi

	go get -d -u "$tests_repo" || true
}

clone_tests_repo

pushd "${tests_repo_dir}"
.ci/setup.sh
popd
