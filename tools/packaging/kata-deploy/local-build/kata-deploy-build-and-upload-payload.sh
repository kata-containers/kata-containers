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

KATA_DEPLOY_DIR="`dirname ${0}`/../../kata-deploy"
KATA_DEPLOY_ARTIFACT="${1:-"kata-static.tar.zst"}"
REGISTRY="${2:-"quay.io/kata-containers/kata-deploy"}"
TAG="${3:-}"

echo "Copying ${KATA_DEPLOY_ARTIFACT} to ${KATA_DEPLOY_DIR}"
cp ${KATA_DEPLOY_ARTIFACT} ${KATA_DEPLOY_DIR}

pushd ${KATA_DEPLOY_DIR}

arch=$(uname -m)
[ "$arch" = "x86_64" ] && arch="amd64"
# Single platform so each job pushes one architecture; attestations (provenance/SBOM)
# are kept by default, making the tag an image index (manifest list).
PLATFORM="linux/${arch}"
IMAGE_TAG="${REGISTRY}:kata-containers-$(git rev-parse HEAD)-${arch}"

echo "Building the image (with provenance and SBOM attestations)"
docker buildx build --platform "${PLATFORM}" \
	--tag "${IMAGE_TAG}" --push .

if [ -n "${TAG}" ]; then
	ADDITIONAL_TAG="${REGISTRY}:${TAG}"

	echo "Building the ${ADDITIONAL_TAG} image"
	docker buildx build --platform "${PLATFORM}" \
		--tag "${ADDITIONAL_TAG}" --push .
fi

popd
