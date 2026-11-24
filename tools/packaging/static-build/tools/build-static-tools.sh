#!/usr/bin/env bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# repo_root_dir is defined in lib.sh, which shellcheck does not follow.
# shellcheck disable=SC2154

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

# Verify that every built binary of a tool is statically linked.
#
# The shared ${repo_root_dir}/target directory may contain more than one
# match (e.g. stale debug/release or per-triple binaries from earlier
# builds), so verify every match rather than assuming a single one.
verify_built_tool() {
	local tool="${1}"
	local tool_binary
	tool_binary="$(get_tool_binary_name "${tool}")"

	local binaries
	binaries="$(find "${repo_root_dir}/target" -type f -name "${tool_binary}")"
	[[ -n "${binaries}" ]] || die "No binary found for ${tool} in ${repo_root_dir}/target (expected: ${tool_binary})"

	local bin
	while IFS= read -r bin; do
		info "Verifying static linkage of ${bin}"
		verify_static_binary "${bin}"
	done <<< "${binaries}"
}

# Select the libc and static-linking settings for the given architecture by
# exporting the variables consumed by make and the Rust build scripts.
# Call it directly, not from a subshell (e.g. $(...)), or the exports are lost.
setup_arch_build_env() {
	local arch="${1}"

	case "${arch}" in
		"s390x"|"ppc64le")
			export LIBC=gnu
			export EXTRA_RUSTFLAGS="-C target-feature=+crt-static"

			# Tell openssl-sys to link libssl/libcrypto statically instead of
			# against shared libraries, so that the resulting binaries pass
			# verify_static_binary() (no ELF interpreter, no NEEDED entries)
			# and can run on hosts that do not provide a matching OpenSSL.
			export OPENSSL_STATIC=1

			# Allow pkg-config-rs to probe pkg-config when explicit target
			# triples (s390x-unknown-linux-gnu, powerpc64le-unknown-linux-gnu)
			# are passed.
			export PKG_CONFIG_ALLOW_CROSS=1
			;;
		*)
			export LIBC=musl
			export EXTRA_RUSTFLAGS="-C link-self-contained=yes"
			;;
	esac
}

build_tool_from_source() {
	local tool="${1}"
	local arch
	arch="$(uname -m)"

	setup_arch_build_env "${arch}"

	/usr/bin/install_libseccomp.sh /opt /opt

	# utils.mk resets EXTRA_RUSTFLAGS, so it must be passed to make as an
	# argument; the environment value alone would be ignored.
	local -a make_args=(EXTRA_RUSTFLAGS="${EXTRA_RUSTFLAGS}")
	# Dragonball VM templates are supported on x86_64.
	if [[ "${tool}" == "kata-ctl" && "${arch}" == "x86_64" ]]; then
		make_args+=(USE_BUILTIN_DB=true)
	fi

	echo "build ${tool} from source"

	cd "${repo_root_dir}/src/tools/${tool}"
	make "${make_args[@]}"

	verify_built_tool "${tool}"
}

build_tool_from_source "$@"
