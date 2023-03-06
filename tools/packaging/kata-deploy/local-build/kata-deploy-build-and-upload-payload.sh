#!/usr/bin/env bash
#
# Copyright 2022 Intel
#
# SPDX-License-Identifier: Apache-2.0
#

KATA_DEPLOY_DIR="`dirname ${0}`/../../kata-deploy"
KATA_DEPLOY_ARTIFACT="${1:-"kata-static.tar.xz"}"
REGISTRY="${2:-"quay.io/kata-containers/kata-deploy"}"
TAG="${3:-}"

echo "Copying ${KATA_DEPLOY_ARTIFACT} to ${KATA_DEPLOY_DIR}"
cp ${KATA_DEPLOY_ARTIFACT} ${KATA_DEPLOY_DIR}

pushd ${KATA_DEPLOY_DIR}

IMAGE_TAG="${REGISTRY}:kata-containers-$(git rev-parse HEAD)-$(uname -m)"

echo "Building the image"
docker build --tag ${IMAGE_TAG} .

echo "Pushing the image to quay.io"
docker push ${IMAGE_TAG}

if [ -n "${TAG}" ]; then
	ADDITIONAL_TAG="${REGISTRY}:${TAG}"

	echo "Building the ${ADDITIONAL_TAG} image"

	docker build --tag ${ADDITIONAL_TAG} .

	echo "Pushing the image ${ADDITIONAL_TAG} to quay.io"
	docker push ${ADDITIONAL_TAG}
fi

popd
