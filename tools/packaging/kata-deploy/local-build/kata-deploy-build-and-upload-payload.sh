#!/usr/bin/env bash
#
# Copyright 2022 Intel
#
# SPDX-License-Identifier: Apache-2.0
#

KATA_DEPLOY_DIR="`dirname ${0}`/../../kata-deploy-cc"
KATA_DEPLOY_ARTIFACT="${1:-"kata-static.tar.xz"}"
REGISTRY="${2:-"quay.io/kata-containers/kata-deploy"}"
TAG="${3:-}"

echo "Copying ${KATA_DEPLOY_ARTIFACT} to ${KATA_DEPLOY_DIR}"
cp ${KATA_DEPLOY_ARTIFACT} ${KATA_DEPLOY_DIR}

pushd ${KATA_DEPLOY_DIR}

IMAGE_TAG="${REGISTRY}:kata-containers-$(git rev-parse HEAD)-$(uname -m)"

echo "Building the image"
case $(uname -m) in
	aarch64)
		docker build \
			--build-arg BASE_IMAGE_NAME=cdocker.io/library/centos \
			--build-arg BASE_IMAGE_TAG=7 \
			--tag ${IMAGE_TAG} .
		;;
	s390x)
		docker build \
			--build-arg BASE_IMAGE_NAME=docker.io/library/clefos \
			--build-arg BASE_IMAGE_TAG=7 \
			--tag ${IMAGE_TAG} .
		;;
	*)
		docker build --tag ${IMAGE_TAG} .
		;;
esac

echo "Pushing the image to quay.io"
docker push ${IMAGE_TAG}

if [ -n "${TAG}" ]; then
	ADDITIONAL_TAG="${REGISTRY}:${TAG}"

	echo "Building the ${ADDITIONAL_TAG} image"

	case $(uname -m) in
		aarch64)
			docker build \
				--build-arg BASE_IMAGE_NAME=docker.io/library/centos  \
				--build-arg BASE_IMAGE_TAG=7 \
				--tag ${ADDITIONAL_TAG} .
			;;
		s390x)
			docker build \
				--build-arg BASE_IMAGE_NAME=docker.io/library/clefos \
				--build-arg BASE_IMAGE_TAG=7 \
				--tag ${ADDITIONAL_TAG} .
			;;
		*)
			docker build --tag ${ADDITIONAL_TAG} .
			;;
	esac

	echo "Pushing the image ${ADDITIONAL_TAG} to quay.io"
	docker push ${ADDITIONAL_TAG}
fi

popd
