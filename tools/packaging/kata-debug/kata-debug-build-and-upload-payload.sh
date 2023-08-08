#!/usr/bin/env bash
#
# Copyright 2023 Intel
#
# SPDX-License-Identifier: Apache-2.0
#

[ -z "${DEBUG}" ] || set -x
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

KATA_DEBUG_DIR="`dirname ${0}`"

REGISTRY="${1:-"quay.io/kata-containers/kata-debug"}"
TAG="${2:-}"

arch=$(uname -m)
[ "$arch" = "x86_64" ] && arch="amd64"
IMAGE_TAG="${REGISTRY}:$(git rev-parse HEAD)-${arch}"

pushd ${KATA_DEBUG_DIR}

echo "Building the image"
docker build --tag ${IMAGE_TAG} .

echo "Pushing the image to the registry"
docker push ${IMAGE_TAG}

if [ -n "${TAG}" ]; then
	ADDITIONAL_TAG="${REGISTRY}:${TAG}"

	echo "Building the ${ADDITIONAL_TAG} image"

	docker build --tag ${ADDITIONAL_TAG} .

	echo "Pushing the image ${ADDITIONAL_TAG} to the registry"
	docker push ${ADDITIONAL_TAG}
fi

popd
