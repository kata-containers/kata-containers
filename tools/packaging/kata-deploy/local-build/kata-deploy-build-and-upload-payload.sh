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

REGISTRY="${1:-"ghcr.io/confidential-dot-ai/kata-deploy"}"
TAG="${2:-}"
ARTIFACTS_BUILD_DIR="${3:-${REPO_ROOT}/tools/packaging/kata-deploy/local-build/build}"
# Separate, minimal image for the job-mode dispatcher (kata-deploy-job-dispatcher).
# Built from its own staged tarball, with the same tag scheme as the kata-deploy
# image. The repo name mirrors the kata-deploy repo with "-job-dispatcher" inserted
# before any "-ci" suffix, so the "-ci" stays last:
#   .../kata-deploy     -> .../kata-deploy-job-dispatcher
#   .../kata-deploy-ci  -> .../kata-deploy-job-dispatcher-ci
if [[ "${REGISTRY}" == *-ci ]]; then
	default_job_dispatcher_image_reference="${REGISTRY%-ci}-job-dispatcher-ci"
else
	default_job_dispatcher_image_reference="${REGISTRY}-job-dispatcher"
fi
JOB_DISPATCHER_IMAGE_REFERENCE="${4:-${default_job_dispatcher_image_reference}}"

KATA_DEPLOY_DIR="${REPO_ROOT}/tools/packaging/kata-deploy"
ARTIFACTS_STAGE_DIR="${KATA_DEPLOY_DIR}/kata-artifacts"

# Stage the component tarballs into a directory that is visible to the
# Docker build context (local-build/ is excluded via .dockerignore).
mkdir -p "${ARTIFACTS_STAGE_DIR}"
cp "${ARTIFACTS_BUILD_DIR}"/kata-static-*.tar.zst "${ARTIFACTS_STAGE_DIR}/"
cp "${ARTIFACTS_BUILD_DIR}"/kata-deploy-static-*.tar.zst "${ARTIFACTS_STAGE_DIR}/"

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
COMMIT_TAG="kata-containers-$(git -C "${REPO_ROOT}" rev-parse HEAD)-${arch}"
IMAGE_TAG="${REGISTRY}:${COMMIT_TAG}"
JOB_DISPATCHER_IMAGE_TAG="${JOB_DISPATCHER_IMAGE_REFERENCE}:${COMMIT_TAG}"

DOCKERFILE="${REPO_ROOT}/tools/packaging/kata-deploy/Dockerfile"
JOB_DISPATCHER_DOCKERFILE="${REPO_ROOT}/tools/packaging/kata-deploy/job-dispatcher/Dockerfile"

echo "Building the kata-deploy image"
docker buildx build --platform "${PLATFORM}" --provenance false --sbom false \
	-f "${DOCKERFILE}" \
	--tag "${IMAGE_TAG}" --push .

echo "Building the kata-deploy-job-dispatcher image"
docker buildx build --platform "${PLATFORM}" --provenance false --sbom false \
	-f "${JOB_DISPATCHER_DOCKERFILE}" \
	--tag "${JOB_DISPATCHER_IMAGE_TAG}" --push .

if [[ -n "${TAG}" ]]; then
	ADDITIONAL_TAG="${REGISTRY}:${TAG}"
	JOB_DISPATCHER_ADDITIONAL_TAG="${JOB_DISPATCHER_IMAGE_REFERENCE}:${TAG}"

	echo "Building the ${ADDITIONAL_TAG} image"
	docker buildx build --platform "${PLATFORM}" --provenance false --sbom false \
		-f "${DOCKERFILE}" \
		--tag "${ADDITIONAL_TAG}" --push .

	echo "Building the ${JOB_DISPATCHER_ADDITIONAL_TAG} image"
	docker buildx build --platform "${PLATFORM}" --provenance false --sbom false \
		-f "${JOB_DISPATCHER_DOCKERFILE}" \
		--tag "${JOB_DISPATCHER_ADDITIONAL_TAG}" --push .
fi

popd
