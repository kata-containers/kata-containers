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

	# On s390x and ppc64le (GNU builds), tell openssl-sys to link libssl/libcrypto
	# statically instead of against shared libraries, so that the resulting
	# binaries pass verify_static_binary() (no ELF interpreter, no NEEDED entries)
	# and can run on hosts that do not provide a matching OpenSSL.
	export OPENSSL_STATIC=1

	# Allow pkg-config-rs to probe pkg-config when explicit target triples
	# (e.g. s390x-unknown-linux-gnu or powerpc64le-unknown-linux-gnu) are passed.
	export PKG_CONFIG_ALLOW_CROSS=1

	/usr/bin/install_libseccomp.sh /opt /opt

	local tool="${1}"
	# ${tool} is the directory name under src/tools/, which is not always
	# the name of the binary cargo produces (e.g. src/tools/agent-ctl builds
	# the kata-agent-ctl crate). Map it so that find() below looks for the
	# right file name.
	local tool_binary="${tool}"
	[[ "${tool}" = "agent-ctl" ]] && tool_binary="kata-agent-ctl"
	[[ "${tool}" = "trace-forwarder" ]] && tool_binary="kata-trace-forwarder"

	echo "build ${tool} from source"

	cd "${repo_root_dir}/src/tools/${tool}"
	make EXTRA_RUSTFLAGS="${extra_rustflags}"

	local binary
	binary="$(find "${repo_root_dir}/target" -type f -name "${tool_binary}")"
	[[ -n "${binary}" ]] || die "No binary found for ${tool} in ${repo_root_dir}/target (expected: ${tool_binary})"

	# The shared ${repo_root_dir}/target directory may contain more than one
	# match (e.g. stale debug/release or per-triple binaries from earlier
	# builds), so verify every match rather than assuming a single one.
	while IFS= read -r bin; do
		[[ -n "${bin}" ]] || continue
		info "Verifying static linkage of ${bin}"
		verify_static_binary "${bin}"
	done <<< "${binary}"
}

build_tool_from_source "$@"
