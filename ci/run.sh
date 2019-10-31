#!/bin/bash
#
# Copyright (c) 2019 Ant Financial
#
# SPDX-License-Identifier: Apache-2.0
#

set -e
cidir=$(dirname "$0")
source "${cidir}/lib.sh"

clone_tests_repo

pushd ${tests_repo_dir}
.ci/run.sh
popd
