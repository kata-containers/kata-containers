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
	export LIBSECCOMP_LIB_PATH=${LIBSECCOMP_LIB_PATH:-/usr/lib}
    export KATA_AGENT_BUILD_TYPE=${KATA_AGENT_BUILD_TYPE:-release}
    export SEALED_SECRET=${SEALED_SECRET:-yes}
    export AGENT_POLICY=${AGENT_POLICY:-yes}

	# This is needed to workaround
	# https://github.com/sfackler/rust-openssl/issues/1624
	export OPENSSL_NO_VENDOR=Y
}

build_agent_from_source() {
	echo "build agent from source"

	init_env

	cd src/agent

	BUILD_TYPE=${KATA_AGENT_BUILD_TYPE} SEALED_SECRET=${SEALED_SECRET} AGENT_POLICY=${AGENT_POLICY} LIBSECCOMP_LINK_TYPE=static LIBSECCOMP_LIB_PATH=${LIBSECCOMP_LIB_PATH} DESTDIR=${DESTDIR} AGENT_POLICY=${AGENT_POLICY} make
	BUILD_TYPE=${KATA_AGENT_BUILD_TYPE} SEALED_SECRET=${SEALED_SECRET} AGENT_POLICY=${AGENT_POLICY} LIBSECCOMP_LINK_TYPE=static LIBSECCOMP_LIB_PATH=${LIBSECCOMP_LIB_PATH} DESTDIR=${DESTDIR} AGENT_POLICY=${AGENT_POLICY} make install
}

build_agent_from_source $@