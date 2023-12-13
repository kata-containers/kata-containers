#!/usr/bin/env bash
#
# Copyright (c) 2023 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#

# Ensure GOPATH set
if command -v go > /dev/null; then
    export GOPATH=${GOPATH:-$(go env GOPATH)}
else
    # if go isn't installed, set default location for GOPATH
    export GOPATH="${GOPATH:-$HOME/go}"
fi

lib_dir=$(dirname "${BASH_SOURCE[0]}")
source "$lib_dir/../../tests/common.bash"

export katacontainers_repo=${katacontainers_repo:="github.com/kata-containers/kata-containers"}
export katacontainers_repo_dir="${GOPATH}/src/${katacontainers_repo}"
