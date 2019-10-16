#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

cidir=$(dirname "$0")
source "${cidir}/lib.sh"

clone_tests_repo

new_goroot=/usr/local/go

pushd "${tests_repo_dir}"
# Force overwrite the current version of golang
[ -z "${GOROOT}" ] || rm -rf "${GOROOT}"
.ci/install_go.sh -p -f -d "$(dirname ${new_goroot})"
[ -z "${GOROOT}" ] || sudo ln -sf "${new_goroot}" "${GOROOT}"
go version
popd
