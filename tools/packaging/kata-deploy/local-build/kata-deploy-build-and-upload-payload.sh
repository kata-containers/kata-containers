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

REGISTRY="${1:-"quay.io/kata-containers/kata-deploy"}"
TAG="${2:-}"

KATA_DEPLOY_DIR="${REPO_ROOT}/tools/packaging/kata-deploy"
ARTIFACTS_BUILD_DIR="${KATA_DEPLOY_DIR}/local-build/build"
ARTIFACTS_STAGE_DIR="${KATA_DEPLOY_DIR}/kata-artifacts"

# Stage the component tarballs into a directory that is visible to the
# Docker build context (local-build/ is excluded via .dockerignore).
mkdir -p "${ARTIFACTS_STAGE_DIR}"
cp "${ARTIFACTS_BUILD_DIR}"/kata-static-*.tar.zst "${ARTIFACTS_STAGE_DIR}/"

cleanup() {
	rm -rf "${ARTIFACTS_STAGE_DIR}"
}
trap cleanup EXIT

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
