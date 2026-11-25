#!/usr/bin/env bash
#
# Copyright (c) 2022 Apple Inc.
#
# SPDX-License-Identifier: Apache-2.0

set -e

cidir=$(dirname "$0")
genpolicydir=${cidir}/../src/tools/genpolicy

build_genpolicy() {
	echo "building genpolicy"
	pushd "${genpolicydir}" &>/dev/null
	make TRIPLE=aarch64-apple-darwin build
}

build_genpolicy
