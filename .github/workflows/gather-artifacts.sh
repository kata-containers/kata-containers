#!/bin/bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o pipefail

pushd kata-artifacts >>/dev/null
for c in ./*.tar.gz
do
    echo "untarring tarball $c"
    tar -xvf $c
done

tar cvfJ ../kata-static.tar.xz ./opt
popd >>/dev/null
