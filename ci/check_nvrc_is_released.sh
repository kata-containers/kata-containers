#!/usr/bin/env bash
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# check_nvrc_is_released.sh - Static check that versions.yaml pins NVRC to a
# published NVIDIA/nvrc release.
#
# A pull request may point externals.nvrc at the artefacts an NVRC pull request
# published (url "oci://ghcr.io/nvidia/nvrc/nvrc-pr", version "<head sha>") to
# test it end to end, but such a pin must never be merged: only released NVRC
# may ship. This check fails for anything but a non-draft, non-prerelease
# release of NVIDIA/nvrc consumed from its GitHub release assets.
#
# Usage: ./ci/check_nvrc_is_released.sh

set -o errexit
set -o nounset
set -o pipefail

[[ -n "${DEBUG:-}" ]] && set -o xtrace

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
versions_yaml="${script_dir}/../versions.yaml"

RELEASE_URL="https://github.com/NVIDIA/nvrc/releases/download/"
RELEASE_API="https://api.github.com/repos/NVIDIA/nvrc/releases/tags"

die() {
	echo -e "FAIL: $*" >&2
	exit 1
}

url="$(yq -r '.externals.nvrc.url' "${versions_yaml}")"
version="$(yq -r '.externals.nvrc.version' "${versions_yaml}")"

echo "versions.yaml pins NVRC ${version} from ${url}"

[[ "${url}" == "${RELEASE_URL}" ]] ||
	die "externals.nvrc.url must be \"${RELEASE_URL}\", got \"${url}\".\nNVRC pull request artefacts can be used for testing, but cannot be merged."

[[ "${version}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] ||
	die "externals.nvrc.version must be an NVRC release tag (vX.Y.Z), got \"${version}\"."

auth=()
token="${GH_TOKEN:-${GITHUB_TOKEN:-}}"
[[ -n "${token}" ]] && auth=(-H "Authorization: Bearer ${token}")

# Drafts are not visible through this endpoint, so they fail as "not found".
release="$(curl -fsSL "${auth[@]}" -H "Accept: application/vnd.github+json" \
	"${RELEASE_API}/${version}")" ||
	die "NVRC ${version} is not a published release of NVIDIA/nvrc."

[[ "$(jq -r '.draft or .prerelease' <<< "${release}")" == "false" ]] ||
	die "NVRC ${version} is a draft or a prerelease, not a release."

echo "OK: NVRC ${version} is a published NVIDIA/nvrc release."
