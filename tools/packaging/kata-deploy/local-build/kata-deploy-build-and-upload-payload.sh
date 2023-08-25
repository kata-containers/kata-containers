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
KATA_DEPLOY_ARTIFACT="${1:-"kata-static.tar.xz"}"
REGISTRY="${2:-"quay.io/confidential-containers/runtime-payload"}"
TAG="${3:-}"
CROSS_BUILD="${4:-}"
TARGET_ARCH="${5:-}"
BUILDX=
PLATFORM=

echo "Copying ${KATA_DEPLOY_ARTIFACT} to ${KATA_DEPLOY_DIR}"
cp ${KATA_DEPLOY_ARTIFACT} ${KATA_DEPLOY_DIR}

pushd ${KATA_DEPLOY_DIR}

arch=$(uname -m)
[ "$arch" = "x86_64" ] && arch="amd64"
IMAGE_TAG="${REGISTRY}:kata-containers-$(git rev-parse HEAD)"

if [ -n "${CROSS_BUILD}" ]; then
	# TAG the image, build and push it
	arch=${TARGET_ARCH}
	BUILDX=buildx
	PLATFORM="--platform=linux/${arch}"
fi


echo "Building the image"
docker ${BUILDX} build ${PLATFORM} --tag ${IMAGE_TAG}-${arch} .

echo "Pushing the image to the registry"
docker push ${IMAGE_TAG}-${arch}

if [ -n "${TAG}" ]; then
	ADDITIONAL_TAG="${REGISTRY}:${TAG}"

	echo "Building the ${ADDITIONAL_TAG} image"
	docker ${BUILDX} build ${PLATFORM} --tag ${ADDITIONAL_TAG} .

	echo "Pushing the image ${ADDITIONAL_TAG} to the registry"
	docker push ${ADDITIONAL_TAG}
fi

popd
