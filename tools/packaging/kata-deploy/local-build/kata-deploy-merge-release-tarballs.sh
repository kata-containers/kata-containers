#!/usr/bin/env bash
# Copyright (c) Kata Contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Merge component tarballs into kata-static.tar.zst and/or kata-go-static.tar.zst
# using per-architecture allowlists.

[[ -z "${DEBUG}" ]] || set -x
set -o errexit
set -o nounset
set -o pipefail

this_script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=tools/packaging/kata-deploy/local-build/kata-release-tarball-inputs.sh
source "${this_script_dir}/kata-release-tarball-inputs.sh"

kata_build_dir="${1:-kata-artifacts}"
kata_versions_yaml_file="${2:-versions.yaml}"
arch="${3:-$(uname -m)}"

merge_with_allowlist() {
	local output="${1}"
	local allowlist="${2}"

	"${this_script_dir}/kata-deploy-merge-builds.sh" \
		"${kata_build_dir}" \
		"${kata_versions_yaml_file}" \
		"${output}" \
		"${allowlist}" \
		merge
}

normalized_arch="$(normalize_arch "${arch}")"

allowlist="$(kata_static_tarball_inputs "${arch}" | tr '\n' ' ')"
merge_with_allowlist "kata-static.tar.zst" "${allowlist}"

allowlist="$(kata_go_tarball_inputs "${arch}" | tr '\n' ' ')"
merge_with_allowlist "kata-go-static.tar.zst" "${allowlist}"
