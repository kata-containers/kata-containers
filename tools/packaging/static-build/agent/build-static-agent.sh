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

	if [[ "${BUILD_VARIANT:-}" == substrate ]]; then
		# Substrate omits policy/init-data but still enforces container seccomp profiles.
		export LIBC=gnu SECCOMP=yes AGENT_POLICY=no INIT_DATA=no USE_DEVMAPPER=no
		# One codegen unit per crate and fat LTO trade build time for optimization.
		export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 CARGO_PROFILE_RELEASE_LTO=true
		# Keep level 3 until steady-state benchmarks justify size optimization; see README.md.
		export CARGO_PROFILE_RELEASE_OPT_LEVEL=3 CARGO_PROFILE_RELEASE_STRIP=symbols
		# shellcheck disable=SC2154
		export CARGO_TARGET_DIR="${repo_root_dir}/target/substrate"
		local target_path
		target_path="${CARGO_TARGET_DIR}/$(uname -m)-unknown-linux-gnu/release/kata-agent"
		make -C src/agent TARGET_PATH="${target_path}" BUILD_TYPE=release INIT=yes
		make -C src/agent TARGET_PATH="${target_path}" BUILD_TYPE=release INIT=yes install
		return
	fi

	# Note: when USE_DEVMAPPER=yes the agent Makefile overrides LIBC=gnu
	cd src/agent
	# shellcheck disable=SC2154
	DESTDIR="${DESTDIR}" AGENT_POLICY="${AGENT_POLICY}" INIT_DATA="${INIT_DATA}" USE_DEVMAPPER="${USE_DEVMAPPER:-no}" make
	DESTDIR="${DESTDIR}" AGENT_POLICY="${AGENT_POLICY}" INIT_DATA="${INIT_DATA}" USE_DEVMAPPER="${USE_DEVMAPPER:-no}" make install
}

build_agent_from_source "$@"
