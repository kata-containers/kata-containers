#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

which bats && exit

BATS_REPO="github.com/bats-core/bats-core"

echo "Install BATS from sources"
go get -d "${BATS_REPO}" || true
pushd "${GOPATH}/src/${BATS_REPO}"
sudo -E PATH=$PATH sh -c "./install.sh /usr"
popd
