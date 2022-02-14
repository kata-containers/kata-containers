#!/usr/bin/env bash
#
# Copyright (c) 2019 Ant Financial
#
# SPDX-License-Identifier: Apache-2.0
#

set -e
cidir=$(dirname "$0")
source "${cidir}/lib.sh"
export CI_JOB="${CI_JOB:-}"

clone_tests_repo

pushd ${tests_repo_dir}
.ci/run.sh
# temporary fix, see https://github.com/kata-containers/tests/issues/3878
if [ "$(uname -m)" != "s390x" ] && [ "$CI_JOB" == "CRI_CONTAINERD_K8S_MINIMAL" ]; then
	tracing/test-agent-shutdown.sh
fi
popd
