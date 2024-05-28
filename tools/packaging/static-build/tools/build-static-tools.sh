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

init_env() {
	source "$HOME/.cargo/env"

	export LIBC=musl
	export LIBSECCOMP_LINK_TYPE=static
	export LIBSECCOMP_LIB_PATH=/usr/lib

	extra_rust_flags=" -C link-self-contained=yes"
}

build_tool_from_source() {
	tool=${1}

	echo "build ${tool} from source"

	cd "src/tools/${tool}"
	make
}

build_tool_from_source "$@"
