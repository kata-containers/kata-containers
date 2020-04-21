#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source "${cidir}/lib.sh"

plugins_version=$(get_version "externals.cni-plugins.commit")
echo "Retrieve CNI plugins repository"
go get -d github.com/containernetworking/plugins || true
pushd $GOPATH/src/github.com/containernetworking/plugins
git checkout "$plugins_version"

echo "Build CNI plugins"
./build_linux.sh

echo "Install CNI binaries"
cni_bin_path="/opt/cni"
sudo mkdir -p ${cni_bin_path}
sudo cp -a bin ${cni_bin_path}

popd

${cidir}/configure_cni.sh
