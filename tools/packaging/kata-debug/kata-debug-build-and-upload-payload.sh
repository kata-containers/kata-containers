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

KATA_DEBUG_DIR="$(readlink -f "$(dirname "${BASH_SOURCE[0]}")")"

REGISTRY="${1:-"quay.io/kata-containers/kata-debug"}"
TAG="${2:-}"

arch=$(uname -m)
[ "$arch" = "x86_64" ] && arch="amd64"
IMAGE_TAG="${REGISTRY}:$(git rev-parse HEAD)-${arch}"

pushd ${KATA_DEBUG_DIR}

cp "${KATA_DEBUG_DIR}/../docker/apt-ci-tune.sh" "${KATA_DEBUG_DIR}/apt-ci-tune.sh"

docker_gha=()
if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
	docker_gha=(--build-arg "GITHUB_ACTIONS=true")
fi

echo "Building the image"
docker build "${docker_gha[@]}" --tag ${IMAGE_TAG} .

echo "Pushing the image to the registry"
docker push ${IMAGE_TAG}

if [ -n "${TAG}" ]; then
	ADDITIONAL_TAG="${REGISTRY}:${TAG}"

	echo "Building the ${ADDITIONAL_TAG} image"

	docker build "${docker_gha[@]}" --tag ${ADDITIONAL_TAG} .

	echo "Pushing the image ${ADDITIONAL_TAG} to the registry"
	docker push ${ADDITIONAL_TAG}
fi

popd
