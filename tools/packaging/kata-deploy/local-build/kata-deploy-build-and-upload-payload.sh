#!/usr/bin/env bash
#
# Copyright 2022 Intel
#
# SPDX-License-Identifier: Apache-2.0
#

[[ -z "${DEBUG}" ]] || set -x
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

SCRIPT_DIR="$(cd "$(dirname "${0}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"
KATA_DEPLOY_DIR="${REPO_ROOT}/tools/packaging/kata-deploy"
STAGED_ARTIFACT="${KATA_DEPLOY_DIR}/kata-static.tar.zst"
KATA_DEPLOY_ARTIFACT="${1:-"kata-static.tar.zst"}"
REGISTRY="${2:-"quay.io/kata-containers/kata-deploy"}"
TAG="${3:-}"

# Only remove a staged copy we created (skip when source is already the staged path).
REMOVE_STAGED_ON_EXIT=false
cleanup() {
	if [[ "${REMOVE_STAGED_ON_EXIT}" = true ]]; then
		rm -f "${STAGED_ARTIFACT}"
	fi
}
trap cleanup EXIT

src_rp="$(realpath -e "${KATA_DEPLOY_ARTIFACT}" 2>/dev/null || true)"
dest_rp="$(realpath -e "${STAGED_ARTIFACT}" 2>/dev/null || true)"
if [[ -n "${src_rp}" ]] && [[ -n "${dest_rp}" ]] && [[ "${src_rp}" = "${dest_rp}" ]]; then
	echo "Artifact already at staged path ${STAGED_ARTIFACT}; skipping copy"
else
	echo "Copying ${KATA_DEPLOY_ARTIFACT} to ${STAGED_ARTIFACT}"
	cp "${KATA_DEPLOY_ARTIFACT}" "${STAGED_ARTIFACT}"
	REMOVE_STAGED_ON_EXIT=true
fi

pushd "${REPO_ROOT}"

arch=$(uname -m)
[[ "${arch}" = "x86_64" ]] && arch="amd64"
[[ "${arch}" = "aarch64" ]] && arch="arm64"
# Disable provenance and SBOM so each tag is a single image manifest. quay.io rejects
# pushing multi-arch manifest lists that include attestation manifests ("manifest invalid").
PLATFORM="linux/${arch}"
IMAGE_TAG="${REGISTRY}:kata-containers-$(git -C "${REPO_ROOT}" rev-parse HEAD)-${arch}"

DOCKERFILE="${REPO_ROOT}/tools/packaging/kata-deploy/Dockerfile"

echo "Building the image"
docker buildx build --platform "${PLATFORM}" --provenance false --sbom false \
	-f "${DOCKERFILE}" \
	--tag "${IMAGE_TAG}" --push .

if [[ -n "${TAG}" ]]; then
	ADDITIONAL_TAG="${REGISTRY}:${TAG}"

	echo "Building the ${ADDITIONAL_TAG} image"
	docker buildx build --platform "${PLATFORM}" --provenance false --sbom false \
		-f "${DOCKERFILE}" \
		--tag "${ADDITIONAL_TAG}" --push .
fi

popd
