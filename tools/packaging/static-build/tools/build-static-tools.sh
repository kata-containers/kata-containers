#!/usr/bin/env bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck source=/dev/null
source "${script_dir}/../../scripts/lib.sh"

build_tool_from_source() {
	export RUSTFLAGS=" -C link-self-contained=yes"
	export LIBC=musl

	/usr/bin/install_libseccomp.sh /opt /opt

	local tool=${1}
	local -a make_args=()
	# Dragonball VM templates are supported on x86_64.
	if [[ "${tool}" == "kata-ctl" && "$(uname -m)" == "x86_64" ]]; then
		make_args+=(USE_BUILTIN_DB=true)
	fi

	echo "build ${tool} from source"

	cd "src/tools/${tool}"
	make "${make_args[@]}"
}

build_tool_from_source "$@"
