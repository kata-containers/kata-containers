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

build_agent_from_source() {
	echo "build agent from source"

	/usr/bin/install_libseccomp.sh /opt /opt

	cd src/agent
	DESTDIR=${DESTDIR} AGENT_POLICY=${AGENT_POLICY} PULL_TYPE=${PULL_TYPE} make
	DESTDIR=${DESTDIR} AGENT_POLICY=${AGENT_POLICY} PULL_TYPE=${PULL_TYPE} make install
}

build_agent_from_source "$@"
