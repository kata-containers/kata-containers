#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

export GOPATH=~/go
export kata_repo="github.com/kata-containers/packaging"
export pr_number=${GITHUB_PR:-}
export pr_branch="PR_${pr_number}"

go get github.com/kata-containers/tests
${GOPATH}/src/github.com/kata-containers/tests/.ci/resolve-kata-dependencies.sh
