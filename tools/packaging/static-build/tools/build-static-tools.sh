#!/usr/bin/env bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "${script_dir}/../../scripts/lib.sh"

build_tool_from_source() {
	RUSTFLAGS=" -C link-self-contained=yes"
	export LIBC=musl

	/usr/bin/install_libseccomp.sh /opt /opt

	tool=${1}

	echo "build ${tool} from source"

	cd "src/tools/${tool}"
	make
}

build_tool_from_source "$@"
