#!/usr/bin/env bash
#
# Copyright 2022 Intel
#
# SPDX-License-Identifier: Apache-2.0
#

[ -z "${DEBUG}" ] || set -x
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

KATA_DEPLOY_DIR="$(dirname "${0}")/../../kata-deploy"
KATA_DEPLOY_ARTIFACT="${1:-"kata-static.tar.zst"}"
REGISTRY="${2:-"quay.io/kata-containers/kata-deploy"}"
TAG="${3:-}"
BUILD_TYPE="${4:-}"

# Determine which Dockerfile to use and build directory
DOCKERFILE="Dockerfile"
BUILD_SUFFIX=""

if [ "${BUILD_TYPE}" = "rust" ]; then
	DOCKERFILE="Dockerfile.rust"
	BUILD_SUFFIX="-rust"
	echo "Building Rust-based kata-deploy image"
else
	echo "Building script-based kata-deploy image (default)"
fi

echo "Copying ${KATA_DEPLOY_ARTIFACT} to ${KATA_DEPLOY_DIR}"
cp "${KATA_DEPLOY_ARTIFACT}" "${KATA_DEPLOY_DIR}"

pushd "${KATA_DEPLOY_DIR}"

arch=$(uname -m)
[ "$arch" = "x86_64" ] && arch="amd64"
IMAGE_TAG="${REGISTRY}:kata-containers-$(git rev-parse HEAD)-${arch}${BUILD_SUFFIX}"

echo "Building the image using ${DOCKERFILE} from $(pwd)"
docker build --file "${DOCKERFILE}" --tag "${IMAGE_TAG}" .

echo "Pushing the image to the registry"
docker push "${IMAGE_TAG}"

if [ -n "${TAG}" ]; then
	ADDITIONAL_TAG="${REGISTRY}:${TAG}${BUILD_SUFFIX}"

	echo "Building the ${ADDITIONAL_TAG} image"

	docker build --file "${DOCKERFILE}" --tag "${ADDITIONAL_TAG}" .

	echo "Pushing the image ${ADDITIONAL_TAG} to the registry"
	docker push "${ADDITIONAL_TAG}"
fi

popd
