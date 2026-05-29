#!/usr/bin/env bash
#
# Copyright (c) 2026 NVIDIA
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root_dir="$(cd "${script_dir}/../../.." && pwd)"

image_ref="${1:?image ref is required (e.g. quay.io/org/repo:tag)}"
shim_tarball="${2:?path to kata-static-shim-v2-go.tar.zst is required}"
push_image="${3:-true}"

case "$(uname -m)" in
	x86_64) platform_arch="amd64" ;;
	aarch64) platform_arch="arm64" ;;
	s390x) platform_arch="s390x" ;;
	ppc64le) platform_arch="ppc64le" ;;
	*) echo "Unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

tmpdir="$(mktemp -d)"
cleanup() {
	rm -rf "${tmpdir}"
}
trap cleanup EXIT

tar --zstd -xf "${shim_tarball}" -C "${tmpdir}"

if [[ ! -x "${tmpdir}/opt/kata/bin/kata-monitor" ]]; then
	echo "kata-monitor binary not found in ${shim_tarball}" >&2
	exit 1
fi

push_flag=()
if [[ "${push_image}" == "true" ]]; then
	push_flag+=(--push)
fi

docker buildx build \
	--platform "linux/${platform_arch}" \
	--provenance false --sbom false \
	-f "${repo_root_dir}/tools/packaging/kata-monitor/Dockerfile" \
	--tag "${image_ref}" \
	"${push_flag[@]}" \
	"${tmpdir}"
