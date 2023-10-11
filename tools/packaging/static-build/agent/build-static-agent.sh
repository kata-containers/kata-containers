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

	# This is needed to workaround
	# https://github.com/sfackler/rust-openssl/issues/1624
	export OPENSSL_NO_VENDOR=Y
}

build_agent_from_source() {
	echo "build agent from source"

	init_env

	cd src/agent
	DESTDIR=${DESTDIR} make
	DESTDIR=${DESTDIR} make install
}

build_agent_from_source $@
