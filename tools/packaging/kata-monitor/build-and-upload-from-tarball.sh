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
source_tarball="${2:?path to kata-static.tar.zst is required}"
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

# Extract only kata-monitor from the source tarball.
monitor_entry=""
while IFS= read -r entry; do
	case "${entry}" in
		opt/kata/bin/kata-monitor|./opt/kata/bin/kata-monitor)
			monitor_entry="${entry}"
			break
			;;
	esac
done < <(tar --zstd -tf "${source_tarball}")

if [[ -z "${monitor_entry}" ]]; then
	echo "kata-monitor binary entry not found in ${source_tarball}" >&2
	exit 1
fi

tar --zstd -xf "${source_tarball}" -C "${tmpdir}" "${monitor_entry}"
monitor_entry="${monitor_entry#./}"
monitor_binary="${tmpdir}/${monitor_entry}"

if [[ ! -f "${monitor_binary}" ]]; then
	echo "kata-monitor binary extraction failed from ${source_tarball}" >&2
	exit 1
fi

chmod +x "${monitor_binary}"

push_flag=()
if [[ "${push_image}" == "true" ]]; then
	push_flag+=(--push)
fi

image_registry_build_args=()
[[ -n "${IMAGE_REGISTRY:-}" ]] && image_registry_build_args=(--build-arg "IMAGE_REGISTRY=${IMAGE_REGISTRY}")

docker buildx build \
	--platform "linux/${platform_arch}" \
	--provenance false --sbom false \
	"${image_registry_build_args[@]}" \
	-f "${repo_root_dir}/tools/packaging/kata-monitor/Dockerfile" \
	--tag "${image_ref}" \
	"${push_flag[@]}" \
	"${tmpdir}"
