#!/usr/bin/env bash
#
# Copyright (c) 2023 Microsoft
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

ARCH=${ARCH:-$(uname -m)}

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "${script_dir}/../../scripts/lib.sh"

[ -n "$opa_repo" ] || die "failed to get OPA repo"
[ -n "$opa_version" ] || die "failed to get OPA version"

[ -d "opa" ] && rm -r opa

pull_opa_released_binary() {
	if [ "${ARCH}" != "aarch64" ] && [ "${ARCH}" != "x86_64" ]; then
		info "Only aarch64 and x86_64 binaries are distributed as part of the OPA releases" && return 1
	fi
	info "Downloading OPA version: ${opa_version}"

	[ -n "$opa_binary_url" ] || opa_binary_url=$(get_from_kata_deps "externals.open-policy-agent.${ARCH}.binary")
	[ -n "$opa_binary_url" ] || die "failed to get OPA binary URL"

	mkdir -p opa

	pushd opa
	curl --fail -L ${opa_binary_url} -o opa || return 1
	chmod +x opa
	popd
}

build_opa_from_source() {
	echo "build OPA from source"

	git clone --depth 1 --branch ${opa_version} ${opa_repo} opa
	pushd opa

	make build WASM_ENABLED=0

	local binary_base_name="opa_linux_"
	case ${ARCH} in
		"aarch64")
			mv -f "${binary_base_name}arm64" ./opa
			;;
		"ppc64le")
			mv -f "${binary_base_name}ppc64le" ./opa
			;;
		"s390x")
			mv -f "${binary_base_name}s390x" ./opa
			;;
		"x86_64")
			mv -f "${binary_base_name}amd64" ./opa
			;;
	esac
	chmod +x opa

	popd
}

#pull_opa_released_binary || build_opa_from_source
pull_opa_released_binary
