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

verify_static_binary() {
	local binary="${1}"

	[[ -f "${binary}" ]] || die "Binary not found: ${binary}"

	if readelf -l "${binary}" | grep -qE 'INTERP|program interpreter'; then
		die "${binary} has an ELF interpreter (dynamically linked)"
	fi

	if readelf -d "${binary}" 2>/dev/null | grep -q '(NEEDED)'; then
		die "${binary} has shared-library dependencies"
	fi
}

build_tool_from_source() {
	local arch extra_rustflags
	arch="$(uname -m)"
	case "${arch}" in
		"s390x"|"ppc64le")
			export LIBC=gnu
			extra_rustflags="-C target-feature=+crt-static"
			;;
		*)
			export LIBC=musl
			extra_rustflags="-C link-self-contained=yes"
			;;
	esac

	export OPENSSL_STATIC=1

	/usr/bin/install_libseccomp.sh /opt /opt

	local tool="${1}"
	local tool_binary="${tool}"
	[[ "${tool}" = "agent-ctl" ]] && tool_binary="kata-agent-ctl"
	[[ "${tool}" = "trace-forwarder" ]] && tool_binary="kata-trace-forwarder"

	echo "build ${tool} from source"

	cd "${repo_root_dir}/src/tools/${tool}"
	make EXTRA_RUSTFLAGS="${extra_rustflags}"

	local binary
	binary="$(find "${repo_root_dir}/target" -type f -name "${tool_binary}")"
	[[ -n "${binary}" ]] || die "No binary found for ${tool} in ${repo_root_dir}/target (expected: ${tool_binary})"

	while IFS= read -r bin; do
		[[ -n "${bin}" ]] || continue
		info "Verifying static linkage of ${bin}"
		verify_static_binary "${bin}"
	done <<< "${binary}"
}

build_tool_from_source "$@"
