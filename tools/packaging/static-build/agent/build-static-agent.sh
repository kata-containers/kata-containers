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

build_agent_from_source() {
	echo "build agent from source"

	/usr/bin/install_libseccomp.sh /opt /opt

	# Note: when USE_DEVMAPPER=yes the agent Makefile overrides LIBC=gnu
	cd src/agent
	# shellcheck disable=SC2154
	DESTDIR="${DESTDIR}" AGENT_POLICY="${AGENT_POLICY}" INIT_DATA="${INIT_DATA}" USE_DEVMAPPER="${USE_DEVMAPPER:-no}" make
	DESTDIR="${DESTDIR}" AGENT_POLICY="${AGENT_POLICY}" INIT_DATA="${INIT_DATA}" USE_DEVMAPPER="${USE_DEVMAPPER:-no}" make install
}

build_agent_from_source "$@"
