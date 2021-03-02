#!/bin/bash
# Copyright (c) 2020 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

install_aarch64_musl() {
	local arch=$(uname -m)
	if [ "${arch}" == "aarch64" ]; then
		local musl_tar="${arch}-linux-musl-native.tgz"
		local musl_dir="${arch}-linux-musl-native"
		pushd /tmp
		if curl -sLO --fail https://musl.cc/${musl_tar}; then
			tar -zxf ${musl_tar}
			mkdir -p /usr/local/musl/
			cp -r ${musl_dir}/* /usr/local/musl/
		fi
		popd
	fi
}

install_aarch64_musl
