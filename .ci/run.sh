#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

cidir=$(dirname "$0")
source "${cidir}/lib.sh"
source /etc/os-release

SNAP_CI="${SNAP_CI:-false}"

pushd "${tests_repo_dir}"

if [ "$SNAP_CI" == "true" ] && [ "$ID" == "ubuntu" ]; then
	export RUNTIME="kata-runtime"
	export CI_JOB="${CI_JOB:-default}"

	echo "INFO: Running checks"
	sudo -E PATH="$PATH" bash -c "make check"

	echo "INFO: Running only supported tests: https://github.com/kata-containers/tests/issues/1495"
	sudo -E PATH="$PATH" bash -c \
		 "make functional docker crio docker-compose network netmon \
		 docker-stability oci openshift kubernetes swarm \
		 entropy ramdisk tracing"
else
	.ci/run.sh
fi
popd

# This script will execute packaging tests suite
