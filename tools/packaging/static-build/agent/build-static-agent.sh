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

	ARCH=$(uname -m)
	rust_arch=""
	case ${ARCH} in
		"aarch64")
			export LIBC=musl
			rust_arch=${ARCH}
			;;
		"ppc64le")
			export LIBC=gnu
			rust_arch="powerpc64le"
			;;
		"x86_64")
			export LIBC=musl
			rust_arch=${ARCH}
			;;
		"s390x")
			export LIBC=gnu
			rust_arch=${ARCH}
			;;
	esac
	rustup target add ${rust_arch}-unknown-linux-${LIBC}

	export LIBSECCOMP_LINK_TYPE=static
	export LIBSECCOMP_LIB_PATH=/usr/lib
}

build_agent_from_source() {
	echo "build agent from source"

	init_env

	/usr/bin/install_libseccomp.sh /usr /usr

	cd src/agent
	DESTDIR=${DESTDIR} AGENT_POLICY=${AGENT_POLICY} PULL_TYPE=${PULL_TYPE} make
	DESTDIR=${DESTDIR} AGENT_POLICY=${AGENT_POLICY} PULL_TYPE=${PULL_TYPE} make install
}

build_agent_from_source "$@"