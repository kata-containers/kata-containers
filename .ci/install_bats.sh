#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

which bats && exit

echo "Install BATS from sources"
go get -d github.com/sstephenson/bats || true
pushd $GOPATH/src/github.com/sstephenson/bats
sudo -E PATH=$PATH sh -c "./install.sh /usr"
popd
