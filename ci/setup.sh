#!/usr/bin/env bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

cidir=$(dirname "$0")
source "${cidir}/lib.sh"

clone_tests_repo

pushd "${tests_repo_dir}"
.ci/setup.sh

env

# local result=$(check_ignore_depends_label)
# if [ "${result}" -eq 1 ]; then
#     echo "Not applying depends on due to '${ignore_depends_on_label}' label"
# else
# 	apply_depends_on
# fi
apply_depends_on

popd
